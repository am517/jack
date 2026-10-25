class Jack < Formula
  desc "Tag and sort movie and TV files using TMDB and TheTVDB metadata"
  homepage "https://github.com/am517/jack"
  url "https://github.com/am517/jack/archive/refs/tags/v0.1.1.tar.gz"
  sha256 "70a752183d9e6ea54ff56d1acd829f103a83e19f47d870a1bb0b416376d3378a"
  license "MIT"
  head "https://github.com/am517/jack.git", branch: "main"

  depends_on "rust" => :build
  depends_on "atomicparsley"
  depends_on "exiftool"
  depends_on "mkvtoolnix"

  def install
    # Optional build-time API keys (never stored in the formula):
    #   HOMEBREW_JACK_TMDB_TOKEN=... HOMEBREW_JACK_TVDB_KEY=... brew install --build-from-source am517/tap/jack
    ENV["JACK_TMDB_TOKEN"] = ENV["HOMEBREW_JACK_TMDB_TOKEN"] if ENV["HOMEBREW_JACK_TMDB_TOKEN"].present?
    ENV["JACK_TVDB_KEY"] = ENV["HOMEBREW_JACK_TVDB_KEY"] if ENV["HOMEBREW_JACK_TVDB_KEY"].present?
    system "cargo", "install", *std_cargo_args
    pkgshare.install "config/jack.toml" => "config.toml.example"
  end

  def caveats
    <<~EOS
      Copy the example config and add your TMDB / TheTVDB API keys:
        mkdir -p ~/.config/windups/jack
        cp #{opt_pkgshare}/config.toml.example ~/.config/windups/jack/config.toml
    EOS
  end

  test do
    assert_match "jack", shell_output("#{bin}/jack --version")
  end
end
