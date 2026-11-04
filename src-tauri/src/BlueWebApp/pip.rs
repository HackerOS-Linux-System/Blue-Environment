pub const PIP_SCRIPT_TEMPLATE: &str = r#"
(function() {
  if (window.top !== window) return;
  if (window.__bluePip) return;
  var TAB = "{TAB_ID}";
  var active = false, current = null, lastSig = '', styleEl = null, timer = null;

  var CSS = [
    'html[data-blue-pip-on]{overflow:hidden!important;background:#000!important}',
    'html[data-blue-pip-on] body{margin:0!important;padding:0!important;overflow:hidden!important;background:#000!important}',
    'html[data-blue-pip-on] body *:not([data-blue-pip]){visibility:hidden!important}',
    'html[data-blue-pip-on] [data-blue-pip="chain"]{background:transparent!important;border:0!important;box-shadow:none!important;',
    'transform:none!important;filter:none!important;backdrop-filter:none!important;perspective:none!important;contain:none!important;',
    'clip-path:none!important;mask:none!important;opacity:1!important;overflow:visible!important;will-change:auto!important;',
    'content-visibility:visible!important;visibility:visible!important}',
    'html[data-blue-pip-on] [data-blue-pip="chain"]::before,html[data-blue-pip-on] [data-blue-pip="chain"]::after{display:none!important}',
    'html[data-blue-pip-on] video[data-blue-pip="video"]{position:fixed!important;top:0!important;left:0!important;right:0!important;bottom:0!important;',
    'width:100vw!important;height:100vh!important;max-width:none!important;max-height:none!important;min-width:0!important;min-height:0!important;',
    'margin:0!important;padding:0!important;border:0!important;transform:none!important;z-index:2147483647!important;object-fit:contain!important;',
    'background:#000!important;visibility:visible!important;opacity:1!important;display:block!important;cursor:default!important}'
  ].join('');

  function inv(args) {
    try {
      var i = window.__TAURI_INTERNALS__;
      if (i && i.invoke) i.invoke('web_pip_report', args).catch(function () {});
    } catch (e) {}
  }

  function area(v) {
    var r = v.getBoundingClientRect();
    var w = Math.max(0, Math.min(r.right, window.innerWidth) - Math.max(r.left, 0));
    var h = Math.max(0, Math.min(r.bottom, window.innerHeight) - Math.max(r.top, 0));
    return w * h;
  }

  function pick() {
    var vs = document.getElementsByTagName('video');
    var best = null, score = -1;
    for (var i = 0; i < vs.length; i++) {
      var v = vs[i];
      var playing = !v.paused && !v.ended;
      if (!playing && !(v.videoWidth > 0)) continue;
      var s = area(v) + (playing ? 1e9 : 0) + (v.videoWidth * v.videoHeight) / 1e3;
      if (s > score) { score = s; best = v; }
    }
    return best;
  }

  function report(force) {
    var v = active && current && document.contains(current) ? current : pick();
    var has = !!v;
    var playing = has && !v.paused && !v.ended;
    var muted = has && (v.muted || v.volume === 0);
    var aspect = has && v.videoWidth > 0 && v.videoHeight > 0 ? v.videoWidth / v.videoHeight : 1.7778;
    var sig = [has, playing, muted, Math.round(aspect * 100)].join('|');
    if (!force && sig === lastSig) return;
    lastSig = sig;
    inv({ tabId: TAB, hasVideo: has, playing: playing, muted: muted, aspect: aspect });
  }

  function clearMarks() {
    var marked = document.querySelectorAll('[data-blue-pip]');
    for (var i = 0; i < marked.length; i++) marked[i].removeAttribute('data-blue-pip');
  }

  function apply() {
    if (!current) return;
    if (!styleEl || !styleEl.isConnected) {
      styleEl = document.createElement('style');
      styleEl.setAttribute('data-blue-pip-style', '1');
      styleEl.textContent = CSS;
      (document.head || document.documentElement).appendChild(styleEl);
    }
    document.documentElement.setAttribute('data-blue-pip-on', '1');
    var el = current.parentElement;
    while (el && el !== document.documentElement) {
      el.setAttribute("data-blue-pip", "chain");
      el = el.parentElement;
    }
    current.setAttribute('data-blue-pip', 'video');
  }

  function tick() {
    if (!active) return;
    var best = pick();
    if (best && best !== current) { clearMarks(); current = best; }
    if (current && !document.contains(current)) { clearMarks(); current = best; }
    apply();
    report(false);
  }

  function onClick(e) {
    if (active && current && e.target === current) {
      if (current.paused) current.play(); else current.pause();
    }
  }

  var api = {
    enter: function () {
      var v = pick();
      if (!v) { report(true); return false; }
      current = v; active = true;
      apply();
      document.addEventListener('click', onClick, true);
      if (timer) clearInterval(timer);
      timer = setInterval(tick, 1000);
      report(true);
      return true;
    },
    exit: function () {
      active = false; current = null;
      if (timer) { clearInterval(timer); timer = null; }
      document.removeEventListener('click', onClick, true);
      clearMarks();
      document.documentElement.removeAttribute('data-blue-pip-on');
      if (styleEl && styleEl.parentNode) styleEl.parentNode.removeChild(styleEl);
      styleEl = null;
      report(true);
    },
    cmd: function (action, val) {
      var v = (current && document.contains(current)) ? current : pick();
      if (!v) return;
      try {
        if (action === 'play') v.play();
        else if (action === 'pause') v.pause();
        else if (action === 'toggle') { if (v.paused) v.play(); else v.pause(); }
        else if (action === 'mute') v.muted = true;
        else if (action === 'unmute') { v.muted = false; if (v.volume === 0) v.volume = 1; }
        else if (action === 'toggle_mute') v.muted = !v.muted;
        else if (action === 'seek') {
          var t = v.currentTime + (typeof val === 'number' ? val : 0);
          var d = isFinite(v.duration) ? v.duration : t;
          v.currentTime = Math.max(0, Math.min(d, t));
        }
      } catch (e) {}
      report(true);
    }
  };
  window.__bluePip = api;

  ['play', 'pause', 'volumechange', 'loadedmetadata', 'emptied', 'ended', 'resize'].forEach(function (name) {
    document.addEventListener(name, function () { report(false); }, true);
  });
  setInterval(function () { if (!active) report(false); }, 2000);
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', function () { report(true); });
  else report(true);
})();
"#;

/// Podmienia id karty w skrypcie.
pub fn script_for_tab(tab_id: &str) -> String {
    // id karty jest generowane przez frontend (`t<liczby>-<znaki a-z0-9>`), ale
    // i tak przepuszczamy tylko bezpieczne znaki — nic nie wyjdzie z literału JS.
    let safe: String = tab_id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    PIP_SCRIPT_TEMPLATE.replace("{TAB_ID}", &safe)
}

/// Ogranicza proporcje obrazu do rozsądnego przedziału (pionowe 9:16 … ultraszerokie).
pub fn clamp_aspect(aspect: f64) -> f64 {
    if !aspect.is_finite() {
        return 16.0 / 9.0;
    }
    aspect.clamp(0.4, 3.0)
}

/// Biała lista poleceń dla odtwarzacza w PiP. Zwraca gotowy kod JS albo `None`,
/// gdy akcja jest nieznana — nic spoza tej listy nie trafia do strony.
pub fn control_script(action: &str, value: Option<f64>) -> Option<String> {
    let val = match value {
        Some(v) if v.is_finite() => v.clamp(-600.0, 600.0),
        _ => 0.0,
    };
    let known = matches!(
        action,
        "play" | "pause" | "toggle" | "mute" | "unmute" | "toggle_mute" | "seek"
    );
    if !known {
        return None;
    }
    Some(format!(
        "window.__bluePip && window.__bluePip.cmd(\"{action}\", {val});"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_id_is_sanitised() {
        let s = script_for_tab("t1-ab\";alert(1);//");
        assert!(s.contains("var TAB = \"t1-abalert1\";"));
        assert!(!s.contains("{TAB_ID}"));
    }

    #[test]
    fn aspect_is_clamped() {
        assert_eq!(clamp_aspect(f64::NAN), 16.0 / 9.0);
        assert_eq!(clamp_aspect(100.0), 3.0);
        assert_eq!(clamp_aspect(0.0), 0.4);
        assert_eq!(clamp_aspect(1.5), 1.5);
    }

    #[test]
    fn only_known_actions_pass() {
        assert!(control_script("toggle", None).is_some());
        assert!(control_script("seek", Some(10.0)).unwrap().contains("\"seek\", 10"));
        assert!(control_script("eval", None).is_none());
        assert!(control_script("play\");alert(1);//", None).is_none());
    }

    #[test]
    fn seek_value_is_bounded() {
        assert!(control_script("seek", Some(1e9)).unwrap().contains("600"));
        assert!(control_script("seek", Some(f64::NAN)).unwrap().contains(", 0)"));
    }
}
