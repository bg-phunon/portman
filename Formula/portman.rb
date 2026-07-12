class Portman < Formula
  desc "TUI tool for monitoring and managing processes listening on TCP ports"
  homepage "https://github.com/bg-phunon/portman"
  url "https://github.com/bg-phunon/portman/archive/refs/tags/v0.2.3.tar.gz"
  sha256 "b47079a4e46ac4d4c1a397475565cee4e1f003468f576bc930a5e0ea4a9b86eb"
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
