import Foundation
import Testing
@testable import OwlBarCore

/// Copie d'une PR enregistrée avec d'autres champs, en repassant par le JSON.
func withFields(_ pr: PullRequest, _ fields: [String: Any]) throws -> PullRequest {
    var object = try #require(
        JSONSerialization.jsonObject(with: try JSONEncoder.owl.encode(EncodablePR(pr))) as? [String: Any]
    )
    for (key, value) in fields { object[key] = value }
    let envelope: [String: Any] = ["version": 1, "ok": true, "data": object]
    return try decodeOwl(PullRequest.self, from: JSONSerialization.data(withJSONObject: envelope)).get()
}

func mergeablePR(base: String = "feature/x", methods: [String] = ["squash", "rebase"]) throws -> PullRequest {
    try withFields(recordedList()[0], ["base_ref": base, "merge_methods": methods])
}

func dialog(_ opening: MergeOpening) throws -> MergeDialog {
    guard case .dialog(let dialog) = opening else {
        Issue.record("fenêtre attendue, refus obtenu : \(opening)")
        throw CancellationError()
    }
    return dialog
}

func texts(_ render: MergeRender) -> [String] {
    render.lines.map { $0.map(\.text).joined() }
}

struct MergeDialogTests {
    @Test func aDraftIsRefusedWithTheMessageOfTheTui() throws {
        let draft = try withFields(recordedList()[0], ["is_draft": true])
        #expect(openMerge(draft, lastUsed: nil, preferred: .squash)
            == .denied("This pull request is a draft, it must be published first."))
    }

    @Test func conflictsAndAnUnknownStateAreRefused() throws {
        let conflicting = try withFields(recordedList()[0], ["mergeable": "conflicting"])
        #expect(openMerge(conflicting, lastUsed: nil, preferred: .squash) == .denied("Conflicts to resolve."))
        let unknown = try withFields(recordedList()[0], ["mergeable": "unknown"])
        #expect(openMerge(unknown, lastUsed: nil, preferred: .squash)
            == .denied("Merge state being computed, try again in a moment."))
    }

    @Test func noAllowedMethodIsRefused() throws {
        #expect(openMerge(try mergeablePR(methods: []), lastUsed: nil, preferred: .squash)
            == .denied("No merge method allowed on this repository."))
    }

    @Test(arguments: ["main", "develop"])
    func aDirectMergeIntoAProtectedBranchStartsWithAWarning(base: String) throws {
        let opened = try dialog(openMerge(try mergeablePR(base: base), lastUsed: nil, preferred: .squash))
        #expect(opened.stage == .warning)
    }

    @Test func anyOtherBranchGoesStraightToTheChoice() throws {
        let opened = try dialog(openMerge(try mergeablePR(), lastUsed: nil, preferred: .squash))
        #expect(opened.stage == .choosing)
    }

    @Test func everyMethodIsListedInOrderWithItsPermission() throws {
        let opened = try dialog(openMerge(try mergeablePR(), lastUsed: nil, preferred: .squash))
        #expect(opened.choices == [
            MergeChoice(method: .merge, allowed: false),
            MergeChoice(method: .squash, allowed: true),
            MergeChoice(method: .rebase, allowed: true),
        ])
    }

    @Test func theLastUsedMethodComesFirstThenThePreferredOneThenTheFirstAllowed() throws {
        let pr = try mergeablePR()
        #expect(try dialog(openMerge(pr, lastUsed: .rebase, preferred: .squash)).method == .rebase)
        #expect(try dialog(openMerge(pr, lastUsed: .merge, preferred: .squash)).method == .squash)
        #expect(try dialog(openMerge(pr, lastUsed: nil, preferred: .merge)).method == .squash)
    }

    @Test func theArrowsSkipRefusedMethodsAndStopAtTheEnds() throws {
        var opened = try dialog(openMerge(try mergeablePR(), lastUsed: nil, preferred: .squash))
        opened.move(down: false)
        #expect(opened.method == .squash, "merge est refusée : rien au-dessus")
        opened.move(down: true)
        #expect(opened.method == .rebase)
        opened.move(down: true)
        #expect(opened.method == .rebase)
    }

    @Test func theWarningIsRedAndNamesTheBranch() throws {
        let render = mergeRender(try dialog(openMerge(try mergeablePR(base: "develop"), lastUsed: nil, preferred: .squash)))
        #expect(render.title == "WARNING")
        #expect(render.frameTone == .red)
        #expect(render.lines[1].first == Segment(text: "develop", tone: .red))
        #expect(texts(render).contains("⚠  DIRECT MERGE INTO DEVELOP  ⚠"))
        #expect(texts(render).contains("This pull request targets develop. Merging it will land directly on develop."))
        #expect(texts(render).last == "Enter to continue · Esc to cancel")
    }

    @Test func theChoiceListsTheMethodsWithTheSelectionMarker() throws {
        let render = mergeRender(try dialog(openMerge(try mergeablePR(), lastUsed: nil, preferred: .squash)))
        #expect(render.title == "Merge")
        #expect(render.frameTone == nil)
        #expect(texts(render) == [
            "#42 Ajoute la fenêtre de fusion",
            "feature/x ← feat/fusion",
            "",
            "Method:",
            "    Create a merge commit",
            "  → Squash and merge",
            "    Rebase and merge",
            "",
            "Enter to confirm · Esc to cancel",
        ])
        #expect(render.lines[4].first?.tone == .gray, "méthode refusée en gris")
    }

    @Test func submittingAndFailingHaveTheirOwnLines() throws {
        var opened = try dialog(openMerge(try mergeablePR(), lastUsed: nil, preferred: .squash))
        opened.stage = .submitting
        #expect(Array(texts(mergeRender(opened)).suffix(3)) == ["Method: squash and merge", "", "Merging…"])
        opened.stage = .failed("Base branch was modified.")
        let failed = mergeRender(opened)
        #expect(Array(texts(failed).suffix(3)) == ["Base branch was modified.", "", "Enter to retry · Esc to close"])
        #expect(failed.lines[3].first?.tone == .red)
    }

    @Test func theButtonsFollowTheStage() throws {
        var opened = try dialog(openMerge(try mergeablePR(base: "main"), lastUsed: nil, preferred: .squash))
        #expect(mergeRender(opened).confirmLabel == "Continue")
        #expect(mergeRender(opened).cancelLabel == "Cancel")
        opened.stage = .choosing
        #expect(mergeRender(opened).confirmLabel == "Squash and merge")
        opened.stage = .submitting
        #expect(mergeRender(opened).confirmLabel == nil)
        #expect(mergeRender(opened).cancelLabel == nil)
        opened.stage = .failed("x")
        #expect(mergeRender(opened).confirmLabel == "Retry")
        #expect(mergeRender(opened).cancelLabel == "Close")
    }
}
