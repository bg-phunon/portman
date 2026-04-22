class Portman < Formula
  desc "TUI tool for monitoring and managing processes listening on TCP ports"
  homepage "https://github.com/bg-phunon/portman"
  url "https://github.com/bg-phunon/portman/archive/refs/tags/v0.2.2.tar.gz"
  sha256 "636dedd9b90b3fd3012e77efe99704f2c2cf82b213a2d33533f6f53acc5b13bc"
  license "MIT"
  head "https://github.com/bg-phunon/portman.git", branch: "main"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    assert_match "portman", shell_output("#{bin}/portman --help 2>&1")
  end
end
