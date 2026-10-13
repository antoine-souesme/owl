import Testing
@testable import OwlBarCore

struct RunnerTests {
    @Test func cargoComesBeforeHomebrewAndUsrLocal() {
        let everywhere: (String) -> Bool = { _ in true }
        #expect(locateOwl(home: "/Users/me", isExecutable: everywhere) == "/Users/me/.cargo/bin/owl")
    }

    @Test func theFirstDirectoryHoldingOwlWins() {
        let onlyLocal: (String) -> Bool = { $0 == "/usr/local/bin/owl" }
        #expect(locateOwl(home: "/Users/me", isExecutable: onlyLocal) == "/usr/local/bin/owl")
        let onlyBrew: (String) -> Bool = { $0 == "/opt/homebrew/bin/owl" }
        #expect(locateOwl(home: "/Users/me", isExecutable: onlyBrew) == "/opt/homebrew/bin/owl")
    }

    @Test func noOwlAnywhereGivesNothing() {
        #expect(locateOwl(home: "/Users/me", isExecutable: { _ in false }) == nil)
    }

    @Test func theChildPathPutsTheSearchDirectoriesFirst() {
        #expect(
            childPath(home: "/Users/me", inherited: "/usr/bin:/bin")
                == "/Users/me/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin"
        )
        #expect(
            childPath(home: "/Users/me", inherited: nil)
                == "/Users/me/.cargo/bin:/opt/homebrew/bin:/usr/local/bin"
        )
    }
}
