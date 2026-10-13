import Foundation
import Testing
@testable import OwlBarCore

/// Faux `owl` : rend la réponse préparée pour la sous-commande, compte les
/// appels, et peut faire attendre la réponse.
final class FakeRunner: OwlRunning, @unchecked Sendable {
    var answers: [String: Result<Data, OwlFailure>] = [:]
    var calls: [[String]] = []
    var delay: UInt64 = 0

    func run(_ arguments: [String]) async -> Result<Data, OwlFailure> {
        calls.append(arguments)
        if delay > 0 { try? await Task.sleep(nanoseconds: delay) }
        return answers[arguments.joined(separator: " ")] ?? .failure(.unreadable)
    }
}

final class FakeNotifier: Notifying, @unchecked Sendable {
    var sent: [(String, String)] = []
    func notify(title: String, body: String) { sent.append((title, body)) }
}

func listAnswer(interval: Int = 60) throws -> Result<Data, OwlFailure> {
    var object = try #require(JSONSerialization.jsonObject(with: fixture("list")) as? [String: Any])
    var data = try #require(object["data"] as? [String: Any])
    data["refresh_interval"] = interval
    object["data"] = data
    return .success(try JSONSerialization.data(withJSONObject: object))
}

func errorAnswer(_ kind: String, _ message: String, resetAt: String? = nil) -> Result<Data, OwlFailure> {
    var error: [String: Any] = ["kind": kind, "message": message]
    if let resetAt { error["reset_at"] = resetAt }
    let object: [String: Any] = ["version": 1, "ok": false, "error": error]
    return .success(try! JSONSerialization.data(withJSONObject: object))
}

func listWith(_ prs: [PullRequest]) throws -> Result<Data, OwlFailure> {
    let encoder = JSONEncoder()
    encoder.keyEncodingStrategy = .convertToSnakeCase
    encoder.dateEncodingStrategy = .iso8601
    let items = try prs.map { try JSONSerialization.jsonObject(with: encoder.encode(EncodablePR($0))) }
    let object: [String: Any] = [
        "version": 1, "ok": true,
        "data": ["pull_requests": items, "preferred_merge_method": "squash", "refresh_interval": 60],
    ]
    return .success(try JSONSerialization.data(withJSONObject: object))
}

@MainActor
final class BarStateTests {
    let runner = FakeRunner()
    let notifier = FakeNotifier()
    var clock = Date(timeIntervalSince1970: 1_800_000_000)
    var state: BarState!

    init() {
        state = BarState(runner: runner, notifier: notifier, now: { [unowned self] in self.clock })
    }

    @Test func beforeTheFirstListTheIconWaits() {
        #expect(state.title == "🦉 …")
    }

    @Test func aListFillsThePanelAndTheCounter() async throws {
        runner.answers["list"] = try listAnswer()
        await state.refresh()
        #expect(state.pullRequests.count == 5)
        #expect(state.errorMessage == nil)
        #expect(state.title == menuBarTitle(counter: counter(state.pullRequests), failed: false))
        #expect(state.lastRefresh == clock)
    }

    @Test func aFailureShowsOwlsMessageAndAlertsTheIcon() async throws {
        runner.answers["list"] = errorAnswer("token", "Not signed in to GitHub. Run `gh auth login`.")
        await state.refresh()
        #expect(state.errorMessage == "Not signed in to GitHub. Run `gh auth login`.")
        #expect(state.title == "🦉 ⚠")
    }

    @Test func anUnreadableAnswerIsAnErrorNotACrash() async {
        runner.answers["list"] = .success(Data("Usage: owl […]".utf8))
        await state.refresh()
        #expect(state.errorMessage == "Unreadable answer from owl.")
        #expect(state.title == "🦉 ⚠")
    }

    @Test func aSuccessAfterAFailureClearsTheAlert() async throws {
        runner.answers["list"] = .failure(.notFound)
        await state.refresh()
        runner.answers["list"] = try listAnswer()
        await state.refresh()
        #expect(state.errorMessage == nil)
    }

    @Test func twoRefreshesAtOnceLaunchOwlOnce() async throws {
        runner.answers["list"] = try listAnswer()
        runner.delay = 50_000_000
        async let first: Void = state.refresh()
        async let second: Void = state.refresh()
        _ = await (first, second)
        #expect(runner.calls.filter { $0 == ["list"] }.count == 1)
    }

    @Test func aRateLimitPausesEveryRefreshUntilItsResetTime() async throws {
        let reset = ISO8601DateFormatter().string(from: clock.addingTimeInterval(600))
        runner.answers["list"] = errorAnswer("rate_limited", "Rate limit reached.", resetAt: reset)
        await state.refresh()
        runner.answers["list"] = try listAnswer()
        clock = clock.addingTimeInterval(300)
        await state.refresh()
        #expect(runner.calls.count == 1, "rien avant l'heure de reprise")
        clock = clock.addingTimeInterval(301)
        await state.refresh()
        #expect(runner.calls.count == 2)
        #expect(state.errorMessage == nil)
    }

    @Test func aZeroIntervalStopsAutomaticRefreshOnly() async throws {
        runner.answers["list"] = try listAnswer(interval: 0)
        await state.refresh()
        #expect(state.nextRefreshDelay == nil)
        await state.refresh()
        #expect(runner.calls.count == 2, "le bouton marche toujours")
    }

    @Test func theIntervalComesFromOwl() async throws {
        #expect(state.nextRefreshDelay == 60, "valeur par défaut avant la première lecture")
        runner.answers["list"] = try listAnswer(interval: 120)
        await state.refresh()
        #expect(state.nextRefreshDelay == 120)
    }

    @Test func aPullRequestBecomingReadyIsNotified() async throws {
        let pr = try recordedList()[0]
        runner.answers["list"] = try listAnswer()
        await state.refresh()
        #expect(notifier.sent.isEmpty, "rien à la première lecture")
        let blocked = try withVerdict(pr, .blocked, number: 4242)
        let ready = try withVerdict(pr, .clean, number: 4242)
        runner.answers["list"] = try listWith([blocked])
        await state.refresh()
        runner.answers["list"] = try listWith([ready])
        await state.refresh()
        #expect(notifier.sent.map(\.0) == ["Ready to merge"])
        #expect(notifier.sent.map(\.1) == [readyBody(ready)])
    }

    @Test func openingAPullRequestLoadsItsDetail() async throws {
        let key = PrKey(repo: "moi/owl", number: 42)
        runner.answers["detail moi/owl 42"] = .success(try fixture("detail"))
        await state.open(key)
        #expect(state.openKey == key)
        guard case .loaded(let detail) = state.detail else {
            Issue.record("détail attendu")
            return
        }
        #expect(detail.summary.number == 42)
    }

    @Test func aLateDetailForAnotherPullRequestIsIgnored() async throws {
        runner.answers["detail moi/owl 42"] = .success(try fixture("detail"))
        runner.delay = 50_000_000
        let state = self.state!
        let first = Task { await state.open(PrKey(repo: "moi/owl", number: 42)) }
        while runner.calls.isEmpty { await Task.yield() }
        state.back()
        runner.delay = 0
        runner.answers["detail moi/notes 7"] = errorAnswer("not_found", "Pull request not found.")
        await state.open(PrKey(repo: "moi/notes", number: 7))
        await first.value
        #expect(state.openKey == PrKey(repo: "moi/notes", number: 7))
        #expect(state.detail == .failed("Pull request not found."))
    }

    @Test func aMergeSuccessGoesBackToAFreshList() async throws {
        let key = PrKey(repo: "moi/owl", number: 42)
        runner.answers["detail moi/owl 42"] = .success(try fixture("detail"))
        runner.answers["merge moi/owl 42 squash"] = .success(Data(#"{"version":1,"ok":true,"data":{}}"#.utf8))
        runner.answers["list"] = try listAnswer()
        await state.open(key)
        await state.merge(.squash)
        #expect(state.openKey == nil)
        #expect(state.mergeStatus == .idle)
        #expect(runner.calls.last == ["list"])
    }

    @Test func aMergeFailureShowsGithubsMessageVerbatim() async throws {
        let key = PrKey(repo: "moi/owl", number: 42)
        runner.answers["detail moi/owl 42"] = .success(try fixture("detail"))
        runner.answers["merge moi/owl 42 rebase"] = errorAnswer("api", "Base branch was modified. Review and try the merge again.")
        await state.open(key)
        await state.merge(.rebase)
        #expect(state.openKey == key)
        #expect(state.mergeStatus == .failed("Base branch was modified. Review and try the merge again."))
    }

    @Test func aLateMergeFailureIsNotShownOnAnotherPullRequest() async throws {
        let first = PrKey(repo: "moi/owl", number: 42)
        let other = PrKey(repo: "moi/notes", number: 7)
        runner.answers["detail moi/owl 42"] = .success(try fixture("detail"))
        runner.answers["merge moi/owl 42 squash"] = errorAnswer("api", "Base branch was modified.")
        await state.open(first)
        runner.delay = 50_000_000
        let state = self.state!
        let merging = Task { await state.merge(.squash) }
        while runner.calls.last?.first != "merge" { await Task.yield() }
        state.back()
        runner.delay = 0
        await state.open(other)
        await merging.value
        #expect(state.openKey == other)
        #expect(state.mergeStatus == .idle)
    }

    @Test func comingBackDuringAMergeStillShowsItRunning() async throws {
        let key = PrKey(repo: "moi/owl", number: 42)
        runner.answers["detail moi/owl 42"] = .success(try fixture("detail"))
        runner.answers["merge moi/owl 42 squash"] = .success(Data(#"{"version":1,"ok":true,"data":{}}"#.utf8))
        runner.answers["list"] = try listAnswer()
        await state.open(key)
        runner.delay = 50_000_000
        let state = self.state!
        let merging = Task { await state.merge(.squash) }
        while runner.calls.last?.first != "merge" { await Task.yield() }
        state.back()
        runner.delay = 0
        await state.open(key)
        #expect(state.mergeStatus == .running)
        await state.merge(.squash)
        #expect(runner.calls.filter { $0.first == "merge" }.count == 1, "une seule fusion à la fois")
        await merging.value
    }

    @Test func theFirstRowIsSelectedAfterTheFirstList() async throws {
        runner.answers["list"] = try listAnswer()
        await state.refresh()
        #expect(state.selectedKey == PrKey(repo: "moi/owl", number: 42))
        #expect(state.rows.map(\.marker) == ["→", "", "", "", ""])
    }

    @Test func theArrowsMoveTheSelectionWithinTheList() async throws {
        runner.answers["list"] = try listAnswer()
        await state.refresh()
        state.selectPrevious()
        #expect(state.selectedKey?.number == 42, "rien au-dessus de la première")
        state.selectNext()
        state.selectNext()
        #expect(state.selectedKey?.number == 13)
        for _ in 0..<10 { state.selectNext() }
        #expect(state.selectedKey?.number == 3, "rien sous la dernière")
    }

    @Test func theSelectionSurvivesARefreshWhileItsPullRequestRemains() async throws {
        runner.answers["list"] = try listAnswer()
        await state.refresh()
        state.selectNext()
        await state.refresh()
        #expect(state.selectedKey?.number == 7)
        runner.answers["list"] = try listWith([try recordedList()[0]])
        await state.refresh()
        #expect(state.selectedKey?.number == 42, "PR sortie : retour à la première")
    }

    @Test func mOpensTheSelectionStraightOnTheMergeConfirmation() async throws {
        runner.answers["list"] = try listAnswer()
        runner.answers["detail moi/owl 42"] = .success(try fixture("detail"))
        await state.refresh()
        await state.startMerge()
        #expect(state.openKey == PrKey(repo: "moi/owl", number: 42))
        #expect(state.confirmingMerge)
    }

    @Test func openingAPullRequestDoesNotAskForConfirmation() async throws {
        runner.answers["detail moi/owl 42"] = .success(try fixture("detail"))
        await state.open(PrKey(repo: "moi/owl", number: 42))
        #expect(!state.confirmingMerge)
        state.requestMerge()
        #expect(state.confirmingMerge)
        state.back()
        #expect(!state.confirmingMerge)
    }

    @Test func noConfirmationWithoutAnAllowedMethod() async throws {
        let pr = try recordedList()[0]
        var object = try #require(JSONSerialization.jsonObject(with: JSONEncoder.owl.encode(EncodablePR(pr))) as? [String: Any])
        object["merge_methods"] = [String]()
        let data: [String: Any] = ["pull_requests": [object], "preferred_merge_method": "squash", "refresh_interval": 60]
        runner.answers["list"] = .success(try JSONSerialization.data(withJSONObject: ["version": 1, "ok": true, "data": data]))
        await state.refresh()
        await state.startMerge()
        #expect(state.openKey == pr.key)
        #expect(!state.confirmingMerge)
    }
}

extension JSONEncoder {
    static var owl: JSONEncoder {
        let encoder = JSONEncoder()
        encoder.keyEncodingStrategy = .convertToSnakeCase
        encoder.dateEncodingStrategy = .iso8601
        return encoder
    }
}
