# Draft formula for the shared Swarm tap (homebrew-swarm). Not published yet.
class Jack < Formula
  desc "Tag and sort movie and TV files (Swarm)"
  homepage "https://github.com/TODO-OWNER/jack" # TODO: real repo URL
  # TODO: point at a tagged release tarball and fill in its sha256
  #   (shasum -a 256 jack-0.1.0.tar.gz).
  url "https://github.com/TODO-OWNER/jack/archive/refs/tags/v0.1.0.tar.gz"
  sha256 "0000000000000000000000000000000000000000000000000000000000000000" # TODO
  license "MIT" # TODO: confirm license
  head "https://github.com/TODO-OWNER/jack.git", branch: "main"

  depends_on "rust" => :build
  depends_on "atomicparsley"
  depends_on "exiftool"
  depends_on "mkvtoolnix"

  def install
    system "cargo", "install", *std_cargo_args
    (etc/"jack").install "config/jack.toml" => "config.toml.example"
  end

  def caveats
    <<~EOS
      Copy the example config and add your TMDB / TheTVDB keys:
        mkdir -p ~/.config/jack
        cp #{etc}/jack/config.toml.example ~/.config/jack/config.toml
    EOS
  end

  test do
    assert_match "jack", shell_output("#{bin}/jack --version")
  end
end
