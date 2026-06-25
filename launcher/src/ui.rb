# frozen_string_literal: true
# Blue Launcher — ANSI colour helpers

def bold(s);   "\e[1m#{s}\e[0m"; end
def green(s);  "\e[32m#{s}\e[0m"; end
def yellow(s); "\e[33m#{s}\e[0m"; end
def red(s);    "\e[31m#{s}\e[0m"; end
def cyan(s);   "\e[36m#{s}\e[0m"; end
def dim(s);    "\e[2m#{s}\e[0m"; end

def ok(m);   puts "  #{green('✓')} #{m}"; end
def err(m);  puts "  #{red('✗')} #{m}"; end
def info(m); puts "  #{cyan('→')} #{m}"; end
def warn(m); puts "  #{yellow('!')} #{m}"; end

def die(m)
  err(m)
  exit(1)
end

def print_hr(len = 54)
  puts "─" * len
end
