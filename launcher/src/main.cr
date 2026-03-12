require "option_parser"

# ====================== ANSI KOLORY (ładny wygląd CLI) ======================
module ANSI
  def self.bold_blue(text : String) : String
    "\e[1;34m#{text}\e[0m"
  end

  def self.blue(text : String) : String
    "\e[34m#{text}\e[0m"
  end

  def self.green(text : String) : String
    "\e[32m#{text}\e[0m"
  end

  def self.yellow(text : String) : String
    "\e[33m#{text}\e[0m"
  end

  def self.red(text : String) : String
    "\e[31m#{text}\e[0m"
  end

  def self.bold_red(text : String) : String
    "\e[1;31m#{text}\e[0m"
  end

  def self.cyan(text : String) : String
    "\e[36m#{text}\e[0m"
  end
end

# ====================== NAGŁÓWEK ======================
def print_header
  puts ANSI.bold_blue("╔══════════════════════════════════════════╗")
  puts ANSI.bold_blue("║         BLUE ENVIRONMENT CLI v0.1.0      ║")
  puts ANSI.bold_blue("╚══════════════════════════════════════════╝")
  puts ""
end

# ====================== POMOC ======================
def usage
  print_header
  puts ANSI.blue("Użycie:")
  puts "  blue start [--dev]   - uruchamia Blue-Environment"
  puts "  blue update          - (placeholder)"
  puts "  blue info            - (placeholder)"
  puts ""
  puts ANSI.yellow("Flaga --dev:")
  puts "    uruchamia ./blue-environment --dev"
  puts "    (wymaga bycia w innej sesji – NIE w czystym TTY)"
  exit 0
end

# ====================== SPRAWDZANIE TTY ======================
def in_tty?
  STDIN.tty?
end

# ====================== GŁÓWNA LOGIKA ======================
print_header

if ARGV.empty? || ARGV[0] == "-h" || ARGV[0] == "--help"
  usage
end

command = ARGV.shift

case command
when "start"
  dev_mode = false

  # Parsowanie flagi --dev (tylko dla tej komendy)
  OptionParser.parse(ARGV) do |parser|
    parser.banner = "blue start [--dev]"
    parser.on("--dev", "Uruchom w trybie deweloperskim") { dev_mode = true }
    parser.on("-h", "--help", "Pokaż pomoc") { usage }
    parser.invalid do |flag|
      puts ANSI.bold_red("BŁĄD: Nieznana flaga #{flag}")
      exit 1
    end
  end

  # === WARUNKI TTY zgodnie z Twoim opisem ===
  if dev_mode
    # --dev → musi być w "innej sesji" (NIE w czystym TTY)
    if in_tty?
      puts ANSI.bold_red("BŁĄD: W trybie --dev nie możesz być w sesji TTY!")
      puts ANSI.red("      Musisz uruchomić to już z wewnątrz jakiejś sesji.")
      exit 1
    end
  else
    # zwykły start → musi być w TTY (nie w innej sesji)
    unless in_tty?
      puts ANSI.bold_red("BŁĄD: Musisz uruchomić 'blue start' w sesji TTY!")
      puts ANSI.red("      Nie jesteś w terminalu (innej sesji).")
      exit 1
    end
  end

  # === ŚCIEŻKA DO ŚRODOWISKA ===
  env_dir = Path[Dir.home, ".hackeros", "Blue-Environment"]
  unless Dir.exists?(env_dir)
    puts ANSI.bold_red("BŁĄD: Katalog ~/.hackeros/Blue-Environment nie istnieje!")
    exit 1
  end

  Dir.chdir(env_dir.to_s)

  binary = "./blue-environment"
  unless File.exists?(binary)
    puts ANSI.bold_red("BŁĄD: Plik #{binary} nie istnieje w katalogu!")
    exit 1
  end
  unless File.executable?(binary)
    puts ANSI.bold_red("BŁĄD: Plik #{binary} nie jest wykonywalny!")
    exit 1
  end

  # === URUCHOMIENIE ===
  puts ANSI.green("✅ Przechodzę do katalogu Blue-Environment...")
  if dev_mode
    puts ANSI.cyan("🔧 Tryb deweloperski (--dev) aktywowany")
    Process.exec(binary, ["--dev"])
  else
    puts ANSI.green("🚀 Uruchamiam Blue Environment...")
    Process.exec(binary)
  end

when "update"
  puts ANSI.yellow("blue update → placeholder (jeszcze nie zaimplementowane)")

when "info"
  puts ANSI.yellow("blue info → placeholder (jeszcze nie zaimplementowane)")

else
  puts ANSI.bold_red("Nieznana komenda: #{command}")
  usage
end
