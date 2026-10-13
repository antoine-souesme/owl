import Testing
@testable import OwlBarCore

struct RunnerTests {
    @Test func owlIsTakenFromTheAppItself() {
        let bundled: (String) -> Bool = { $0 == "/Applications/Owl.app/Contents/MacOS/owl" }
        #expect(
            locateOwl(executableDirectory: "/Applications/Owl.app/Contents/MacOS", isExecutable: bundled)
                == "/Applications/Owl.app/Contents/MacOS/owl"
        )
    }

    @Test func anOwlInstalledElsewhereIsIgnored() {
        let installedOnly: (String) -> Bool = { $0 == "/Users/me/.cargo/bin/owl" }
        #expect(locateOwl(executableDirectory: "/Applications/Owl.app/Contents/MacOS", isExecutable: installedOnly) == nil)
    }

    @Test func noExecutableDirectoryGivesNothing() {
        #expect(locateOwl(executableDirectory: nil, isExecutable: { _ in true }) == nil)
    }

    @Test func theMissingOwlMessageSaysHowToRebuild() {
        #expect(OwlFailure.notFound.message == "owl is missing from the app. Rebuild it with `macos/build.sh`.")
    }

    @Test func theChildPathPutsTheDirectoriesOfGhFirst() {
        #expect(childPath(inherited: "/usr/bin:/bin") == "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin")
        #expect(childPath(inherited: nil) == "/opt/homebrew/bin:/usr/local/bin")
    }
}
