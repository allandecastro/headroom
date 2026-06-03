cask "headroom" do
  version "1.5.1"
  sha256 "237c3f77258d67786717cf7f6032916077db899ea5ff1dedc8df87c6199b8d79"

  url "https://github.com/allandecastro/headroom/releases/download/v#{version}/Headroom_#{version}_aarch64.dmg",
      verified: "github.com/allandecastro/headroom/"
  name "Headroom"
  desc "Menu bar quota meter for Claude Code and GitHub Copilot"
  homepage "https://github.com/allandecastro/headroom"

  livecheck do
    url :url
    strategy :github_latest
  end

  # The release pipeline only ships an Apple Silicon (aarch64) build.
  depends_on arch: :arm64
  depends_on macos: ">= :big_sur"

  app "Headroom.app"

  # Headroom is currently ad-hoc signed, not notarized through the Apple
  # Developer Program. Homebrew quarantines downloaded apps by default, and an
  # un-notarized app under quarantine is what Gatekeeper reports to users as
  # "Headroom is damaged and can't be opened." Stripping the quarantine flag
  # right after install lets the app launch cleanly. Remove this block once the
  # macOS build is Developer ID signed + notarized.
  postflight do
    system_command "/usr/bin/xattr",
                   args: ["-dr", "com.apple.quarantine", "#{appdir}/Headroom.app"]
  end

  uninstall quit: "app.headroom"

  zap trash: [
    "~/Library/Application Support/headroom",
    "~/Library/Caches/app.headroom",
    "~/Library/HTTPStorages/app.headroom",
    "~/Library/Preferences/app.headroom.plist",
    "~/Library/Saved Application State/app.headroom.savedState",
    "~/Library/WebKit/app.headroom",
  ]
end
