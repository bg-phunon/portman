class Portman < Formula
  desc "TUI tool for monitoring and managing processes listening on TCP ports"
  homepage "https://github.com/bg-phunon/portman"
  url "https://github.com/bg-phunon/portman/archive/refs/tags/v0.1.0.tar.gz"
  sha256 "0c4b21d8fa7dace126edb883e70107efb88bb0dec620e1ad3bd7f8b4546be48b"
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
