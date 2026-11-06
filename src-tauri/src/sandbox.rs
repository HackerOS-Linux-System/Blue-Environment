#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

use std::path::{Path, PathBuf};

/// Quotes a string for POSIX `sh`.
pub fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

#[derive(Debug, Clone)]
pub struct SandboxCtx {
    /// `$XDG_RUNTIME_DIR` (e.g. `/run/user/1000`).
    pub runtime_dir: Option<PathBuf>,
    /// `$WAYLAND_DISPLAY` socket name.
    pub wayland_display: Option<String>,
    /// Expose the X11 socket directory.
    pub x11: bool,
    /// Host path of the *filtered* bus socket created by `xdg-dbus-proxy`, if used.
    pub dbus_proxy_sock: Option<PathBuf>,
    /// Compiled seccomp filter to load (`--seccomp 9`, fd 9 opened by the wrapper).
    pub seccomp_file: Option<PathBuf>,
    pub share_net: bool,
    /// Directories replaced by an empty tmpfs. bwrap cannot create a mount
    /// point under the read-only root, so use [`existing_hide_dirs`] when launching.
    pub hide: Vec<String>,
}

impl Default for SandboxCtx {
    fn default() -> Self {
        Self { runtime_dir: None, wayland_display: None, x11: true, dbus_proxy_sock: None, seccomp_file: None, share_net: true, hide: DEFAULT_HIDE.iter().map(|d| d.to_string()).collect() }
    }
}

/// Removable media and mount points: other people's data.
pub const DEFAULT_HIDE: &[&str] = &["/mnt", "/media", "/run/media"];

/// The entries of [`DEFAULT_HIDE`] that exist on this machine.
pub fn existing_hide_dirs() -> Vec<String> {
    DEFAULT_HIDE.iter().filter(|d| Path::new(d).is_dir()).map(|d| d.to_string()).collect()
}

/// Device nodes an ordinary desktop app may legitimately need.
const DEVICE_BINDS: &[&str] = &[
    "/dev/dri", "/dev/snd", "/dev/nvidiactl", "/dev/nvidia0", "/dev/nvidia1", "/dev/nvidia-modeset", "/dev/nvidia-uvm", "/dev/nvidia-uvm-tools",
    "/dev/video0", "/dev/video1", "/dev/video2", "/dev/video3", "/dev/shm",
];

/// Parents of home directories that are replaced by an empty tmpfs so other
/// users' homes (and the real one) are not visible.
const HOME_PARENTS: &[&str] = &["/home", "/var/home"];

/// Bus names the filtered session bus lets sandboxed apps talk to / own.
pub const DBUS_TALK: &[&str] = &["org.freedesktop.portal.*", "org.freedesktop.Notifications", "org.kde.StatusNotifierWatcher", "org.freedesktop.ScreenSaver"];
pub const DBUS_OWN: &[&str] = &["org.mpris.MediaPlayer2.*"];

/// `bwrap` arguments (everything before the program to run).
pub fn hardened_args(real_home: &Path, clone_home: &Path, ctx: &SandboxCtx) -> Vec<String> {
    let s = |p: &Path| p.to_string_lossy().into_owned();
    let mut a: Vec<String> = Vec::new();
    fn push(a: &mut Vec<String>, items: &[&str]) {
        a.extend(items.iter().map(|x| x.to_string()));
    }

    push(&mut a, &["--ro-bind", "/", "/"]);
    push(&mut a, &["--dev", "/dev"]);
    for d in DEVICE_BINDS {
        push(&mut a, &["--dev-bind-try", d, d]);
    }
    push(&mut a, &["--proc", "/proc"]);
    push(&mut a, &["--tmpfs", "/tmp"]);
    if ctx.x11 {
        push(&mut a, &["--bind-try", "/tmp/.X11-unix", "/tmp/.X11-unix"]);
    }
    // other people's / removable data
    for d in &ctx.hide {
        a.extend(["--tmpfs".to_string(), d.clone()]);
    }
    if let Some(parent) = real_home.parent().map(s) {
        if HOME_PARENTS.contains(&parent.as_str()) {
            a.extend(["--tmpfs".to_string(), parent]);
        }
    }
    // the clone's private home, in the real home's place
    a.extend(["--bind".to_string(), s(clone_home), s(real_home)]);
    // read-only bits of the real profile the app still needs
    for rel in [".local/share/flatpak", ".Xauthority", ".local/share/fonts", ".config/fontconfig"] {
        let p = s(&real_home.join(rel));
        a.extend(["--ro-bind-try".to_string(), p.clone(), p]);
    }

    if let Some(rt) = &ctx.runtime_dir {
        let rt_s = s(rt);
        a.extend(["--tmpfs".to_string(), rt_s.clone()]);
        let mut sockets: Vec<String> = vec!["pipewire-0".into(), "pulse".into()];
        if let Some(w) = &ctx.wayland_display {
            // a name, never a path: stops `../` from reaching other files
            if !w.is_empty() && !w.contains('/') {
                sockets.push(w.clone());
            }
        }
        for sock in sockets {
            let p = s(&rt.join(sock));
            a.extend(["--bind-try".to_string(), p.clone(), p]);
        }
        if let Some(proxy) = &ctx.dbus_proxy_sock {
            let target = s(&rt.join("bus"));
            a.extend(["--bind".to_string(), s(proxy), target.clone()]);
            a.extend(["--setenv".to_string(), "DBUS_SESSION_BUS_ADDRESS".to_string(), format!("unix:path={target}")]);
        }
    }
    if ctx.dbus_proxy_sock.is_none() {
        // no bus at all beats the raw bus
        a.extend(["--unsetenv".to_string(), "DBUS_SESSION_BUS_ADDRESS".to_string()]);
    }

    for flag in ["--unshare-pid", "--unshare-ipc", "--unshare-uts", "--unshare-cgroup-try", "--new-session"] {
        a.push(flag.to_string());
    }
    if !ctx.share_net {
        a.push("--unshare-net".to_string());
    }
    a.extend(["--cap-drop".to_string(), "ALL".to_string()]);
    if ctx.seccomp_file.is_some() {
        a.extend(["--seccomp".to_string(), "9".to_string()]);
    }
    a.push("--die-with-parent".to_string());
    a
}

/// Complete `sh` script: starts the filtered D-Bus proxy (when wanted), runs
/// `bwrap … sh -c <exec>` with the seccomp filter on fd 9, then cleans up.
pub fn wrapper_script(exec: &str, real_home: &Path, clone_home: &Path, id: &str, ctx: &SandboxCtx) -> String {
    let args: Vec<String> = hardened_args(real_home, clone_home, ctx).iter().map(|x| sh_quote(x)).collect();
    let mut bw = format!("bwrap {} sh -c {}", args.join(" "), sh_quote(exec));
    if let Some(f) = &ctx.seccomp_file {
        bw.push_str(&format!(" 9<{}", sh_quote(&f.to_string_lossy())));
    }
    let Some(sock) = &ctx.dbus_proxy_sock else { return bw };
    let sock_q = sh_quote(&sock.to_string_lossy());
    let mut proxy = format!("xdg-dbus-proxy \"$DBUS_SESSION_BUS_ADDRESS\" {sock_q} --filter");
    for t in DBUS_TALK {
        proxy.push_str(&format!(" --talk={}", sh_quote(t)));
    }
    for o in DBUS_OWN {
        proxy.push_str(&format!(" --own={}", sh_quote(o)));
    }
    proxy.push_str(" --call='org.freedesktop.portal.*=*' --broadcast='org.freedesktop.portal.*=@/org/freedesktop/portal/*'");
    let _ = id;
    format!(
        "rm -f {sock_q}; {proxy} & BLUE_PROXY=$!; i=0; while [ ! -S {sock_q} ] && [ $i -lt 60 ]; do sleep 0.05; i=$((i+1)); done; {bw}; rc=$?; kill $BLUE_PROXY 2>/dev/null; rm -f {sock_q}; exit $rc"
    )
}

// ── seccomp ────────────────────────────────────────────────────────────────

/// A classic-BPF seccomp deny-list.
///
/// It removes kernel attack surface a desktop app never needs — loading
/// modules, `kexec`, `bpf`, `perf_event_open`, `userfaultfd`, kernel keyrings,
/// mounting, `reboot`, `process_vm_writev`, clock / hostname changes… — and
/// answers `EPERM`. Ordinary programs, browsers and Electron apps (which need
/// `clone`/`unshare` for their own sandboxes and `ptrace` for crash reporting)
/// are unaffected.
///
/// Only the native ABI is filtered; the x32 ABI is refused outright. 32-bit
/// (i386 compat) processes are not filtered — pick a different isolation mode
/// for those.
pub mod seccomp {
    const BPF_LD_W_ABS: u16 = 0x20;
    const BPF_JEQ_K: u16 = 0x15;
    const BPF_JGE_K: u16 = 0x35;
    const BPF_RET_K: u16 = 0x06;
    pub const SECCOMP_RET_ALLOW: u32 = 0x7fff_0000;
    pub const SECCOMP_RET_ERRNO_EPERM: u32 = 0x0005_0000 | 1;
    pub const X32_SYSCALL_BIT: u32 = 0x4000_0000;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Insn {
        pub code: u16,
        pub jt: u8,
        pub jf: u8,
        pub k: u32,
    }

    #[cfg(target_arch = "x86_64")]
    pub const NATIVE_AUDIT_ARCH: Option<u32> = Some(0xC000_003E);
    #[cfg(target_arch = "aarch64")]
    pub const NATIVE_AUDIT_ARCH: Option<u32> = Some(0xC000_00B7);
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    pub const NATIVE_AUDIT_ARCH: Option<u32> = None;

    /// (name, x86_64 number, aarch64 number) — numbers from the kernel headers.
    pub const DENIED: &[(&str, Option<u32>, Option<u32>)] = &[
        ("kexec_load", Some(246), Some(104)),
        ("kexec_file_load", Some(320), Some(294)),
        ("mount", Some(165), Some(40)),
        ("umount2", Some(166), Some(39)),
        ("pivot_root", Some(155), Some(41)),
        ("swapon", Some(167), Some(224)),
        ("swapoff", Some(168), Some(225)),
        ("reboot", Some(169), Some(142)),
        ("init_module", Some(175), Some(105)),
        ("finit_module", Some(313), Some(273)),
        ("delete_module", Some(176), Some(106)),
        ("bpf", Some(321), Some(280)),
        ("perf_event_open", Some(298), Some(241)),
        ("userfaultfd", Some(323), Some(282)),
        ("add_key", Some(248), Some(217)),
        ("request_key", Some(249), Some(218)),
        ("keyctl", Some(250), Some(219)),
        ("acct", Some(163), Some(89)),
        ("settimeofday", Some(164), Some(170)),
        ("clock_adjtime", Some(305), Some(266)),
        ("syslog", Some(103), Some(116)),
        ("lookup_dcookie", Some(212), Some(18)),
        ("nfsservctl", Some(180), Some(42)),
        ("open_by_handle_at", Some(304), Some(265)),
        ("name_to_handle_at", Some(303), Some(264)),
        ("kcmp", Some(312), Some(272)),
        ("iopl", Some(172), None),
        ("ioperm", Some(173), None),
        ("quotactl", Some(179), Some(60)),
        ("vhangup", Some(153), Some(58)),
        ("setdomainname", Some(171), Some(162)),
        ("sethostname", Some(170), Some(161)),
        ("process_vm_writev", Some(311), Some(271)),
        ("move_pages", Some(279), Some(239)),
    ];

    fn numbers(audit_arch: u32) -> Vec<u32> {
        let x86 = audit_arch == 0xC000_003E;
        DENIED.iter().filter_map(|(_, a, b)| if x86 { *a } else { *b }).collect()
    }

    /// The filter program for `audit_arch` (0xC000003E = x86_64, 0xC00000B7 = aarch64).
    pub fn program(audit_arch: u32) -> Vec<Insn> {
        let nrs = numbers(audit_arch);
        let x86 = audit_arch == 0xC000_003E;
        let mut p: Vec<Insn> = Vec::new();
        // 0: load arch; foreign arch → allow (see module doc)
        p.push(Insn { code: BPF_LD_W_ABS, jt: 0, jf: 0, k: 4 });
        p.push(Insn { code: BPF_JEQ_K, jt: 1, jf: 0, k: audit_arch });
        p.push(Insn { code: BPF_RET_K, jt: 0, jf: 0, k: SECCOMP_RET_ALLOW });
        // load syscall number
        p.push(Insn { code: BPF_LD_W_ABS, jt: 0, jf: 0, k: 0 });
        let first_check = p.len();
        let checks = nrs.len() + usize::from(x86);
        // index of the final DENY instruction
        let deny_idx = first_check + checks + 1;
        let push_jump = |p: &mut Vec<Insn>, code: u16, k: u32| {
            let here = p.len();
            let jt = (deny_idx - (here + 1)) as u8;
            p.push(Insn { code, jt, jf: 0, k });
        };
        if x86 {
            push_jump(&mut p, BPF_JGE_K, X32_SYSCALL_BIT); // x32 ABI: refuse everything
        }
        for nr in nrs {
            push_jump(&mut p, BPF_JEQ_K, nr);
        }
        p.push(Insn { code: BPF_RET_K, jt: 0, jf: 0, k: SECCOMP_RET_ALLOW });
        p.push(Insn { code: BPF_RET_K, jt: 0, jf: 0, k: SECCOMP_RET_ERRNO_EPERM });
        p
    }

    /// The byte stream `bwrap --seccomp FD` expects (`struct sock_filter[]`, native endian).
    pub fn to_bytes(prog: &[Insn]) -> Vec<u8> {
        let mut out = Vec::with_capacity(prog.len() * 8);
        for i in prog {
            out.extend_from_slice(&i.code.to_ne_bytes());
            out.push(i.jt);
            out.push(i.jf);
            out.extend_from_slice(&i.k.to_ne_bytes());
        }
        out
    }

    /// Filter for the architecture this binary runs on (`None` on others).
    pub fn native_filter() -> Option<Vec<u8>> {
        NATIVE_AUDIT_ARCH.map(|a| to_bytes(&program(a)))
    }

    /// Minimal cBPF interpreter, enough to run `program` (used by the tests).
    #[cfg(test)]
    pub fn run(prog: &[Insn], arch: u32, nr: u32) -> u32 {
        let mut acc: u32 = 0;
        let mut pc = 0usize;
        while pc < prog.len() {
            let i = prog[pc];
            pc += 1;
            match i.code {
                BPF_LD_W_ABS => acc = if i.k == 4 { arch } else { nr },
                BPF_JEQ_K => {
                    if acc == i.k {
                        pc += i.jt as usize
                    } else {
                        pc += i.jf as usize
                    }
                }
                BPF_JGE_K => {
                    if acc >= i.k {
                        pc += i.jt as usize
                    } else {
                        pc += i.jf as usize
                    }
                }
                BPF_RET_K => return i.k,
                _ => return SECCOMP_RET_ERRNO_EPERM,
            }
        }
        SECCOMP_RET_ERRNO_EPERM
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }
    fn pos(args: &[String], seq: &[&str]) -> Option<usize> {
        args.windows(seq.len()).position(|w| w.iter().zip(seq).all(|(a, b)| a == b))
    }
    fn ctx() -> SandboxCtx {
        SandboxCtx { runtime_dir: Some("/run/user/1000".into()), wayland_display: Some("wayland-1".into()), ..Default::default() }
    }

    #[test]
    fn system_is_read_only_and_home_is_the_clones() {
        let a = hardened_args(Path::new("/home/u"), Path::new("/home/u/.local/share/blue-clones/x/home"), &ctx());
        assert!(pos(&a, &["--ro-bind", "/", "/"]) == Some(0), "the root bind must come first and be read-only");
        assert!(!a.contains(&"--dev-bind".to_string()), "no writable root / raw device access");
        assert!(pos(&a, &["--bind", "/home/u/.local/share/blue-clones/x/home", "/home/u"]).is_some());
        // the /home tmpfs must be laid down BEFORE the clone home is bound into it
        assert!(pos(&a, &["--tmpfs", "/home"]).unwrap() < pos(&a, &["--bind", "/home/u/.local/share/blue-clones/x/home", "/home/u"]).unwrap());
        for hidden in ["/mnt", "/media", "/run/media", "/tmp"] {
            assert!(pos(&a, &["--tmpfs", hidden]).is_some(), "{hidden}");
        }
    }

    #[test]
    fn non_standard_home_locations_are_not_blanked() {
        let a = hardened_args(Path::new("/srv/people/u"), Path::new("/srv/people/u/c"), &ctx());
        assert!(pos(&a, &["--tmpfs", "/srv/people"]).is_none());
        let silverblue = hardened_args(Path::new("/var/home/u"), Path::new("/var/home/u/c"), &ctx());
        assert!(pos(&silverblue, &["--tmpfs", "/var/home"]).is_some());
    }

    #[test]
    fn isolation_flags_are_all_present() {
        let a = hardened_args(Path::new("/home/u"), Path::new("/c"), &ctx());
        for f in ["--unshare-pid", "--unshare-ipc", "--unshare-uts", "--new-session", "--die-with-parent"] {
            assert!(a.contains(&f.to_string()), "{f}");
        }
        assert!(pos(&a, &["--cap-drop", "ALL"]).is_some());
        assert!(!a.contains(&"--unshare-net".to_string()), "network stays shared by default");
        let no_net = hardened_args(Path::new("/home/u"), Path::new("/c"), &SandboxCtx { share_net: false, ..ctx() });
        assert!(no_net.contains(&"--unshare-net".to_string()));
    }

    #[test]
    fn runtime_dir_is_emptied_and_only_display_audio_sockets_come_back() {
        let a = hardened_args(Path::new("/home/u"), Path::new("/c"), &ctx());
        assert!(pos(&a, &["--tmpfs", "/run/user/1000"]).is_some());
        for sock in ["pipewire-0", "pulse", "wayland-1"] {
            let p = format!("/run/user/1000/{sock}");
            assert!(pos(&a, &["--bind-try", &p, &p]).is_some(), "{sock}");
        }
        // a hostile WAYLAND_DISPLAY cannot smuggle a path in
        let evil = hardened_args(Path::new("/home/u"), Path::new("/c"), &SandboxCtx { wayland_display: Some("../secrets".into()), ..ctx() });
        assert!(!evil.iter().any(|x| x.contains("secrets")));
    }

    #[test]
    fn session_bus_is_never_the_raw_bus() {
        let none = hardened_args(Path::new("/home/u"), Path::new("/c"), &ctx());
        assert!(pos(&none, &["--unsetenv", "DBUS_SESSION_BUS_ADDRESS"]).is_some(), "no proxy → no bus");
        let proxied = hardened_args(Path::new("/home/u"), Path::new("/c"), &SandboxCtx { dbus_proxy_sock: Some("/run/user/1000/blue-clone-x.bus".into()), ..ctx() });
        assert!(pos(&proxied, &["--bind", "/run/user/1000/blue-clone-x.bus", "/run/user/1000/bus"]).is_some());
        assert!(pos(&proxied, &["--setenv", "DBUS_SESSION_BUS_ADDRESS", "unix:path=/run/user/1000/bus"]).is_some());
        assert!(pos(&proxied, &["--bind-try", "/run/user/1000/bus", "/run/user/1000/bus"]).is_none(), "the host's own bus socket is never bound in");
    }

    #[test]
    fn wrapper_script_filters_the_bus_and_loads_seccomp() {
        let plain = wrapper_script("firefox --new-instance", Path::new("/home/u"), Path::new("/c"), "x", &ctx());
        assert!(plain.starts_with("bwrap ") && plain.ends_with("sh -c 'firefox --new-instance'"));
        let full = wrapper_script(
            "it's quoted",
            Path::new("/home/u"),
            Path::new("/c"),
            "x",
            &SandboxCtx { dbus_proxy_sock: Some("/run/user/1000/blue-clone-x.bus".into()), seccomp_file: Some("/cache/app.bpf".into()), ..ctx() },
        );
        assert!(full.contains("xdg-dbus-proxy \"$DBUS_SESSION_BUS_ADDRESS\" '/run/user/1000/blue-clone-x.bus' --filter"));
        assert!(full.contains("--talk='org.freedesktop.portal.*'"));
        assert!(!full.contains("--talk=*") && !full.contains("org.freedesktop.secrets"), "the vault is not reachable");
        assert!(full.contains("'--seccomp' '9'") && full.contains("9<'/cache/app.bpf'"));
        assert!(full.contains("'it'\\''s quoted'"), "exec is quoted safely");
        assert!(full.contains("kill $BLUE_PROXY") && full.contains("exit $rc"));
    }

    #[test]
    fn seccomp_denies_exactly_the_listed_syscalls() {
        for (arch, is_x86) in [(0xC000_003Eu32, true), (0xC000_00B7u32, false)] {
            let prog = seccomp::program(arch);
            let mut denied = 0;
            for (name, x, a) in seccomp::DENIED {
                let Some(nr) = (if is_x86 { *x } else { *a }) else { continue };
                assert_eq!(seccomp::run(&prog, arch, nr), seccomp::SECCOMP_RET_ERRNO_EPERM, "{name} must be denied");
                denied += 1;
            }
            assert!(denied >= 30);
            // everyday syscalls pass (x86_64 numbers; on aarch64 these are just arbitrary allowed numbers)
            for ok in [0u32, 1, 2, 3, 9, 56, 59, 101 /* ptrace */, 435 /* clone3 */, 272 /* unshare on x86 */] {
                if !is_x86 && seccomp::DENIED.iter().any(|(_, _, a)| *a == Some(ok)) {
                    continue;
                }
                if is_x86 && seccomp::DENIED.iter().any(|(_, x, _)| *x == Some(ok)) {
                    continue;
                }
                assert_eq!(seccomp::run(&prog, arch, ok), seccomp::SECCOMP_RET_ALLOW, "syscall {ok} must pass");
            }
        }
    }

    #[test]
    fn seccomp_refuses_x32_and_ignores_foreign_arches() {
        let prog = seccomp::program(0xC000_003E);
        assert_eq!(seccomp::run(&prog, 0xC000_003E, seccomp::X32_SYSCALL_BIT | 1), seccomp::SECCOMP_RET_ERRNO_EPERM);
        assert_eq!(seccomp::run(&prog, 0x4000_0003 /* i386 */, 165), seccomp::SECCOMP_RET_ALLOW);
    }

    #[test]
    fn seccomp_program_serialises_to_8_byte_instructions_with_valid_jumps() {
        for arch in [0xC000_003Eu32, 0xC000_00B7u32] {
            let prog = seccomp::program(arch);
            let bytes = seccomp::to_bytes(&prog);
            assert_eq!(bytes.len(), prog.len() * 8);
            assert!(prog.len() < 255, "u8 jump offsets must reach the DENY at the end");
            for (i, ins) in prog.iter().enumerate() {
                assert!(i + 1 + (ins.jt as usize) < prog.len() + 1, "jump out of range at {i}");
            }
        }
    }

    #[test]
    fn args_are_stable_text() {
        let a = hardened_args(Path::new("/home/u"), Path::new("/c"), &ctx());
        assert_eq!(&a[..3], s(&["--ro-bind", "/", "/"]).as_slice());
    }
}
