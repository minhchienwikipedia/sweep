cask "sweep" do
  version "0.1.0"
  sha256 "9402f8cffa5432b3b9eb754406d1f47fd7f18b2700a6d55217606af5db4fc2d3"

  url "https://github.com/minhchienwikipedia/sweep/releases/download/v#{version}/Sweep-#{version}.dmg"
  name "Sweep"
  desc "Fast, minimal disk cleaner for caches, developer tools and project folders"
  homepage "https://github.com/minhchienwikipedia/sweep"

  depends_on arch: :arm64
  depends_on :macos

  app "Sweep.app"

  # The app is ad-hoc signed, not notarized, so clear the download quarantine
  # to let it open without the "Apple could not verify…" prompt.
  postflight_steps do
    run "/usr/bin/xattr", args: ["-dr", "com.apple.quarantine", "{{appdir}}/Sweep.app"]
  end

  uninstall quit: "com.minhchien.sweep"

  zap trash: [
    "~/Library/Caches/com.minhchien.sweep",
    "~/Library/WebKit/com.minhchien.sweep",
  ]
end
