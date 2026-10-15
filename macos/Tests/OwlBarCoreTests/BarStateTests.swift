import Foundation
import Testing
@testable import OwlBarCore

/// Faux `owl` : rend la réponse préparée pour la sous-commande, compte les
/// appels, et peut faire attendre la réponse.
final class FakeRunner: OwlRunning, @unchecked Sendable {
    var answers: [String: Result<Data, OwlFailure>] = [:]
    var calls: [[String]] = []
    var delay: UInt64 = 0
    /// Délai propre à une commande, à la place de `delay`.
    var delays: [String: UInt64] = [:]

    func run(_ arguments: [String]) async -> Result<Data, OwlFailure> {
        calls.append(arguments)
        let delay = delays[arguments.joined(separator: " ")] ?? delay
        if delay > 0 { try? await Task.sleep(nanoseconds: delay) }
        return answers[arguments.joined(separator: " ")] ?? .failure(.unreadable)
    }
}

final class FakeBrowser: @unchecked Sendable {
    var opened: [URL] = []
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
    let browser = FakeBrowser()
    var clock = Date(timeIntervalSince1970: 1_800_000_000)
    var state: BarState!

    init() {
        state = BarState(
            runner: runner,
            notifier: notifier,
            openURL: { [browser] in browser.opened.append($0) },
            now: { [unowned self] in self.clock }
        )
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

    @Test func theFirstRowIsSelectedAfterTheFirstList() async throws {
        runner.answers["list"] = try listAnswer()
        await state.refresh()
        #expect(state.selectedKey == PrKey(repo: "moi/owl", number: 42))
        #expect(state.rows.map(\.isSelected) == [true, false, false, false, false])
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

    @Test func aTickTurnsTheRunningChecks() async throws {
        runner.answers["list"] = try listAnswer()
        await state.refresh()
        #expect(state.isAnimating, "la PR #13 a des vérifications en cours")
        let before = state.rows.first { $0.number == "#13" }?.checks.symbol
        state.tick()
        let after = state.rows.first { $0.number == "#13" }?.checks.symbol
        #expect(before == "⠋")
        #expect(after == "⠙")
    }

    @Test func nothingTurnsWithoutARunningCheck() async throws {
        runner.answers["list"] = try listWith([try recordedList()[0]])
        await state.refresh()
        #expect(!state.isAnimating)
    }

    // MARK: fenêtre de fusion

    let mergeOk: Result<Data, OwlFailure> = .success(Data(#"{"version":1,"ok":true,"data":{}}"#.utf8))

    /// Liste d'une seule PR fusionnable, sur une branche sans avertissement.
    func loadMergeable(base: String = "feature/x") async throws -> PullRequest {
        let pr = try mergeablePR(base: base)
        runner.answers["list"] = try listWith([pr])
        await state.refresh()
        return pr
    }

    @Test func mOnADraftShowsWhyAndOpensNothing() async throws {
        runner.answers["list"] = try listAnswer()
        await state.refresh()
        state.selectNext()
        await state.press(.m)
        #expect(state.mergeDialog == nil)
        #expect(state.notice == "This pull request is a draft, it must be published first.")
        state.selectNext()
        #expect(state.notice == nil, "le message ne survit pas à la touche suivante")
    }

    @Test func enterOnTheWarningLeadsToTheChoice() async throws {
        _ = try await loadMergeable(base: "main")
        await state.press(.m)
        #expect(state.mergeDialog?.stage == .warning)
        await state.press(.enter)
        #expect(state.mergeDialog?.stage == .choosing)
        #expect(!runner.calls.contains { $0.first == "merge" })
    }

    @Test func arrowsChooseTheMethodAndEnterMerges() async throws {
        let pr = try await loadMergeable()
        runner.answers["merge moi/owl 42 rebase"] = mergeOk
        runner.answers["list"] = try listWith([])
        await state.press(.m)
        await state.press(.down)
        await state.press(.enter)
        #expect(runner.calls.contains(["merge", "moi/owl", "42", "rebase"]))
        #expect(state.mergeDialog == nil)
        #expect(state.notice == "moi/owl #42 merged")
        #expect(!state.pullRequests.contains { $0.key == pr.key })
        #expect(runner.calls.last == ["list"])
    }

    @Test func theMethodJustUsedIsPreselectedNextTime() async throws {
        _ = try await loadMergeable()
        runner.answers["merge moi/owl 42 rebase"] = mergeOk
        await state.press(.m)
        await state.press(.down)
        await state.press(.enter)
        _ = try await loadMergeable()
        await state.press(.m)
        #expect(state.mergeDialog?.method == .rebase)
    }

    @Test func aFailureStaysInTheWindowVerbatimAndEnterRetries() async throws {
        _ = try await loadMergeable()
        runner.answers["merge moi/owl 42 squash"] = errorAnswer("api", "Base branch was modified. Review and try the merge again.")
        await state.press(.m)
        await state.press(.enter)
        #expect(state.mergeDialog?.stage == .failed("Base branch was modified. Review and try the merge again."))
        runner.answers["merge moi/owl 42 squash"] = mergeOk
        await state.press(.enter)
        #expect(state.mergeDialog == nil)
        #expect(runner.calls.filter { $0.first == "merge" }.count == 2)
    }

    @Test func escapeClosesTheWindowButNotWhileMerging() async throws {
        _ = try await loadMergeable()
        await state.press(.m)
        await state.press(.escape)
        #expect(state.mergeDialog == nil)

        runner.answers["merge moi/owl 42 squash"] = mergeOk
        await state.press(.m)
        runner.delay = 50_000_000
        let state = self.state!
        let merging = Task { await state.press(.enter) }
        while runner.calls.last?.first != "merge" { await Task.yield() }
        await state.press(.escape)
        #expect(state.mergeDialog?.stage == .submitting, "la fusion est partie")
        await merging.value
    }

    @Test func whileTheWindowIsOpenTheArrowsDoNotMoveTheList() async throws {
        runner.answers["list"] = try listWith([try mergeablePR(), try recordedList()[1]])
        await state.refresh()
        await state.press(.m)
        await state.press(.down)
        #expect(state.selectedKey?.number == 42)
    }


    @Test func mOpensTheMergeWindowOverTheList() async throws {
        _ = try await loadMergeable()
        await state.press(.m)
        #expect(state.mergeDialog?.stage == .choosing)
        #expect(state.mergeRendering?.title == "Merge")
    }

    @Test func clickingARowSelectsItAndOpensTheMergeWindow() async throws {
        runner.answers["list"] = try listWith([try recordedList()[1], try mergeablePR()])
        await state.refresh()
        state.click(PrKey(repo: "moi/owl", number: 42))
        #expect(state.selectedKey?.number == 42)
        #expect(state.mergeDialog?.key.number == 42)
    }

    @Test func enterOnTheListOpensTheMergeWindowToo() async throws {
        _ = try await loadMergeable()
        await state.press(.enter)
        #expect(state.mergeDialog?.stage == .choosing)
    }

    @Test func theGithubButtonOpensThatPullRequest() async throws {
        runner.answers["list"] = try listAnswer()
        await state.refresh()
        state.openOnGitHub(PrKey(repo: "moi/owl", number: 42))
        #expect(browser.opened == [URL(string: try recordedList()[0].url)!])
        #expect(state.mergeDialog == nil)
    }

    @Test func oOpensTheSelectionOnGithub() async throws {
        runner.answers["list"] = try listAnswer()
        await state.refresh()
        await state.press(.down)
        await state.press(.o)
        #expect(browser.opened == [URL(string: try recordedList()[1].url)!])
    }

    @Test func theKeysOfTheList() async throws {
        runner.answers["list"] = try listAnswer()
        await state.press(.r)
        #expect(state.pullRequests.count == 5)
        await state.press(.down)
        await state.press(.down)
        await state.press(.up)
        #expect(state.selectedKey?.number == 7)
    }

    @Test func thePanelOpensOnPullRequests() {
        #expect(state.tab == .pullRequests)
    }

    @Test func commandTwoShowsTheServersAndReadsThem() async {
        runner.answers["ports"] = portsAnswer([])
        await state.press(.tab2)
        #expect(state.tab == .servers)
        #expect(runner.calls == [["ports"]])
        await state.press(.tab1)
        #expect(state.tab == .pullRequests)
        #expect(runner.calls.count == 1)
    }

    @Test func reopeningThePanelOnServersReadsThemAgain() async {
        runner.answers["ports"] = portsAnswer([])
        await state.panelOpened()
        #expect(runner.calls.isEmpty)
        await state.selectTab(.servers)
        await state.panelOpened()
        #expect(runner.calls == [["ports"], ["ports"]])
    }

    @Test func onTheServersTabKeysGoToTheServers() async throws {
        runner.answers["list"] = try listAnswer()
        await state.refresh()
        runner.answers["ports"] = portsAnswer([])
        await state.selectTab(.servers)
        let before = state.selectedKey
        await state.press(.down)
        await state.press(.m)
        #expect(state.selectedKey == before)
        #expect(state.mergeDialog == nil)
        state.servers.query = "abc"
        await state.press(.enter)
        #expect(state.servers.placeholder == "Not a port: abc")
    }

    @Test func tabKeysAreIgnoredWhileTheMergeWindowIsOpen() async throws {
        runner.answers["list"] = try listAnswer()
        await state.refresh()
        state.startMerge()
        try #require(state.mergeDialog != nil)
        await state.press(.tab2)
        #expect(state.tab == .pullRequests)
    }

    @Test func theCounterStillCountsPullRequestsOnly() async throws {
        runner.answers["list"] = try listAnswer()
        await state.refresh()
        let title = state.title
        runner.answers["ports"] = portsAnswer([serverObject(port: 3000, pid: 10)])
        await state.selectTab(.servers)
        #expect(state.title == title)
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
