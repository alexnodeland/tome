# The cask, authored here and living in `alexnodeland/homebrew-tap`.
#
# **`version` and `sha256` below are placeholders and are NOT what ships.** The
# tap bumps its own casks on a schedule, reading this repository's latest
# release — so a release here needs no credential for that repository, and the
# two lines drift here on purpose. Everything else is authored in this file and
# copied across by hand when it changes, which is rare.
#
# Tome ships UNSIGNED and un-notarized (ADR-0006: the Apple Developer Program
# is deferred). Gatekeeper therefore blocks first launch, and the caveats carry
# the fix. Revisit at v1.0.
#
# They are NOT `:latest` / `:no_check`: Tome is unsigned, so the checksum is
# the only integrity check a user gets, and turning it off would remove the
# last one. `brew upgrade` also skips `:latest` casks unless you pass
# `--greedy`. The two lines are matched by a fixed pattern in the tap's
# `scripts/bump.py`, and `scripts/verify-bundle.sh` asserts that pattern still
# matches — so reformatting this file cannot silently break the bumper.
cask "tome" do
  version "0.0.0"
  sha256 "0000000000000000000000000000000000000000000000000000000000000000"

  url "https://github.com/alexnodeland/tome/releases/download/v#{version}/Tome-#{version}.dmg"
  name "Tome"
  desc "Personal library for technical documentation"
  homepage "https://github.com/alexnodeland/tome"

  livecheck do
    url :url
    strategy :github_latest
  end

  depends_on macos: :monterey

  app "Tome.app"
  # The CLI ships INSIDE the bundle (S4-9) rather than as a second artifact, so
  # that `tome` and the app are the same build and therefore resolve the same
  # library — the invariant in ADR-0002. A cask that installed the app and
  # fetched the CLI separately would look identical and drift on the next
  # release.
  binary "#{appdir}/Tome.app/Contents/MacOS/tome"

  # Must match `tome status` and the PRD's File System Layout. Every path here
  # has been observed on a machine that ran Tome, and scripts/verify-bundle.sh
  # fails if the library's own paths are missing from this list.
  #
  # Deliberately NOT listed:
  #   ~/Library/Mobile Documents/iCloud~com~alexnodeland~tome
  #     Sync does not exist, so no version of Tome has ever created this. Add
  #     it in the same change that ships sync.
  #   ~/Library/Caches/tome-app, ~/Library/WebKit/tome-app
  #     Created by the unbundled binary during development. An installed user
  #     never has them.
  zap trash: [
    "~/Library/Application Support/Tome",
    "~/Library/Caches/com.alexnodeland.tome",
    "~/Library/Caches/Tome",
    "~/Library/HTTPStorages/com.alexnodeland.tome",
    "~/Library/Preferences/com.alexnodeland.tome.plist",
    "~/Library/Saved Application State/com.alexnodeland.tome.savedState",
    "~/Library/WebKit/com.alexnodeland.tome",
  ]

  caveats <<~EOS
    Tome is not signed by Apple, so macOS will refuse the first launch.
    To allow it:

      xattr -dr com.apple.quarantine /Applications/Tome.app

    (macOS 15 removed the Control-click then Open bypass, so the command
    above is the way, not a shortcut for it.)

    The `tome` command line tool is installed alongside the app and shares
    its library:

      tome add https://doc.rust-lang.org/std/
      tome pull rust-std
      tome search "hash map"

    Your library lives in ~/Library/Application Support/Tome and cached
    documentation in ~/Library/Caches/Tome.

    `brew uninstall --zap tome` removes both. It cannot remove the API
    token, which is in the Keychain — run this first if you ever started
    the HTTP API:

      tome config forget-token
  EOS
end
