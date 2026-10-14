import Foundation
import Testing
@testable import OwlBarCore

func recordedList() throws -> [PullRequest] {
    try decodeOwl(ListData.self, from: fixture("list")).get().pullRequests
}

struct PresentationTests {
    let now = ISO8601DateFormatter().date(from: "2026-08-30T12:00:00Z")!

    @Test func theAgeGoesFromMinutesToHoursToDays() {
        #expect(shortAge(from: now.addingTimeInterval(-59 * 60), now: now) == "59m")
        #expect(shortAge(from: now.addingTimeInterval(-60 * 60), now: now) == "1h")
        #expect(shortAge(from: now.addingTimeInterval(-25 * 3600), now: now) == "1d")
    }

    @Test func aDateInTheFutureGivesZero() {
        #expect(shortAge(from: now.addingTimeInterval(300), now: now) == "0m")
    }

    @Test func eachStateHasTheGlyphOfTheTui() {
        #expect(checksGlyph(.success) == Glyph(symbol: "✓", tone: .green))
        #expect(checksGlyph(.failure) == Glyph(symbol: "✗", tone: .red))
        #expect(checksGlyph(.pending).tone == .yellow)
        #expect(checksGlyph(.none) == Glyph(symbol: "·", tone: .gray))
        #expect(reviewGlyph(.approved) == Glyph(symbol: "✓", tone: .green))
        #expect(reviewGlyph(.changesRequested) == Glyph(symbol: "✗", tone: .red))
        #expect(reviewGlyph(.reviewRequired) == Glyph(symbol: "●", tone: .yellow))
        #expect(reviewGlyph(.none) == Glyph(symbol: "·", tone: .gray))
    }

    @Test func theCounterSplitsReadyFromTheRest() throws {
        let prs = try recordedList()
        let ready = prs.filter { $0.mergeState == .clean }.count
        #expect(counter(prs) == Counter(ready: ready, others: prs.count - ready))
    }

    @Test func theMenuBarTitle() {
        #expect(menuBarTitle(counter: nil, failed: false) == "🦉 …")
        #expect(menuBarTitle(counter: Counter(ready: 2, others: 5), failed: false) == "🦉 2 · 5")
        #expect(menuBarTitle(counter: Counter(ready: 2, others: 5), failed: true) == "🦉 ⚠")
    }

    @Test func aRowCarriesTheColumnsOfTheTui() throws {
        let row = try #require(rows(recordedList(), now: now).first)
        #expect(row.repo == "moi/owl")
        #expect(row.number == "#42")
        #expect(row.target == "develop")
        #expect(row.arrow == "←")
        #expect(row.source == "feat/fusion")
        #expect(row.title == "Ajoute la fenêtre de fusion")
        #expect(row.age == "2h")
        #expect(row.checks == checksGlyph(.success))
        #expect(row.flags == [])
    }

    @Test func draftsAndConflictsAreFlagged() throws {
        let draft = try #require(rows(recordedList(), now: now).first { $0.number == "#7" })
        #expect(draft.flags.contains("draft"))
        let conflicting = try recordedList().filter { $0.mergeable == .conflicting }
        #expect(!conflicting.isEmpty)
        for row in rows(conflicting, now: now) {
            #expect(row.flags.contains("⚠ conflicts"))
        }
    }

    @Test func theNoMethodMessageMatchesTheTui() {
        #expect(noMergeMethodMessage == "No merge method allowed on this repository.")
    }

    @Test func methodLabelsMatchTheTui() {
        #expect(methodLabel(.squash) == "Squash and merge")
        #expect(methodLabel(.rebase) == "Rebase and merge")
        #expect(methodLabel(.merge) == "Create a merge commit")
    }

    @Test func theNotificationText() throws {
        let pr = try recordedList()[0]
        #expect(readyTitle == "Ready to merge")
        #expect(readyBody(pr) == "moi/owl #42 · Ajoute la fenêtre de fusion")
    }

    @Test func eachCheckOfTheLastCommitHasItsLineUnderTheRow() throws {
        let all = rows(try recordedList(), now: now)
        let row = try #require(all.first { $0.number == "#13" })
        #expect(row.checkLines == [
            CheckLine(connector: "⎿", glyph: checksGlyph(.success), name: "build"),
            CheckLine(connector: "⎿", glyph: checksGlyph(.pending), name: "test"),
            CheckLine(connector: "⎿", glyph: checksGlyph(.pending), name: "ci/deploy-preview"),
        ])
    }

    @Test func aPullRequestWithoutChecksHasNoLineBelow() throws {
        let row = try #require(rows(recordedList(), now: now).first)
        #expect(row.checkLines.isEmpty)
    }

    @Test func theColumnsKeepTheColoursOfTheTui() {
        #expect(Palette.repo == .cyan)
        #expect(Palette.number == .gray)
        #expect(Palette.age == .gray)
        #expect(Palette.target == .blue)
        #expect(Palette.arrow == .gray)
        #expect(Palette.source == .blue)
        #expect(Palette.connector == .gray)
        #expect(Palette.sectionTitle == .cyan)
    }

    @Test func aDraftRowIsDimmedLikeInTheTui() throws {
        let all = rows(try recordedList(), now: now)
        #expect(all.first { $0.number == "#7" }?.dim == true)
        #expect(all.first { $0.number == "#42" }?.dim == false)
    }

    @Test func aRunningCheckTurnsWithTheFramesOfTheTui() {
        let frames = (0..<11).map { checksGlyph(.pending, frame: $0).symbol }
        #expect(frames == ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏", "⠋"])
        #expect(checksGlyph(.success, frame: 3) == checksGlyph(.success))
    }

    @Test func rowsAndChecksFollowTheAnimationFrame() throws {
        let row = try #require(rows(try recordedList(), now: now, frame: 2).first { $0.number == "#13" })
        #expect(row.checks.symbol == "⠹")
        #expect(row.checkLines.map(\.glyph.symbol) == ["✓", "⠹", "⠹"])
    }
}

struct VersionLabelTests {
    @Test func theVersionIsPrefixedWithV() {
        #expect(versionLabel("0.1.6") == "v0.1.6")
    }

    @Test func noVersionShowsNothing() {
        #expect(versionLabel(nil) == "")
    }
}
