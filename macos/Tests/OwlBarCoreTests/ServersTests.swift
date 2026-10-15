import Foundation
import Testing
@testable import OwlBarCore

func serverObject(port: Int, pid: Int, critical: String? = nil) -> [String: Any] {
    [
        "port": port, "pid": pid, "project": "erp-menage",
        "directory": "/Users/moi/erp-menage", "command": "npm run dev",
        "started_at": "2026-10-15T10:00:00Z", "user": "moi",
        "critical": critical.map { ["reason": $0] as Any } ?? NSNull(),
    ]
}

func portsAnswer(_ servers: [[String: Any]], hidden: Bool = false) -> Result<Data, OwlFailure> {
    let object: [String: Any] = ["version": 1, "ok": true, "data": ["servers": servers, "hidden": hidden]]
    return .success(try! JSONSerialization.data(withJSONObject: object))
}

func stopAnswer() throws -> Result<Data, OwlFailure> {
    .success(try fixture("stop"))
}

@MainActor
final class ServersTests {
    let runner = FakeRunner()
    let clock = ISO8601DateFormatter().date(from: "2026-10-15T12:00:00Z")!
    var state: ServersState!

    init() {
        state = ServersState(runner: runner, now: { [clock] in clock })
    }

    @Test func theRecordedListBecomesRows() async throws {
        runner.answers["ports"] = .success(try fixture("ports"))
        await state.reload()
        #expect(state.rows == [
            ServerRow(id: ServerId(pid: 35860, port: 3000), port: "3000", project: "erp-menage",
                      directory: "/Users/antoinesouesme/dev/projets/gkfa/erp-menage",
                      command: "npm run dev", age: "15h", stopLabel: "Stop", isStopping: false),
            ServerRow(id: ServerId(pid: 41022, port: 5173), port: "5173", project: "site",
                      directory: "/Users/antoinesouesme/dev/playground/owl/site",
                      command: "npm run dev", age: "12m", stopLabel: "Stop", isStopping: false),
        ])
        #expect(state.placeholder == nil)
    }

    @Test func aMissingProjectShowsADash() {
        let rows = serverRows([try! decodeOwl(PortsData.self, from: Data(#"{"version":1,"ok":true,"data":{"servers":[{"port":7000,"pid":1,"project":null,"directory":null,"command":"ControlCenter","started_at":"2026-10-15T11:00:00Z","user":"moi","critical":null}],"hidden":false}}"#.utf8)).get().servers[0]], now: clock, stopping: [])
        #expect(rows[0].project == "—")
        #expect(rows[0].directory == nil)
    }

    @Test func anEmptyListSaysSo() async {
        runner.answers["ports"] = portsAnswer([])
        #expect(state.placeholder == nil)
        await state.reload()
        #expect(state.placeholder == "No node server running")
    }

    @Test func aFailureShowsOwlsMessage() async {
        runner.answers["ports"] = errorAnswer("system", "lsof: boom")
        await state.reload()
        #expect(state.placeholder == "lsof: boom")
    }

    @Test func aReadIsNeverDoubled() async {
        runner.answers["ports"] = portsAnswer([])
        runner.delay = 50_000_000
        async let first: Void = state.reload()
        async let second: Void = state.reload()
        _ = await (first, second)
        #expect(runner.calls.count == 1)
    }

    @Test func stoppingFromTheListIsImmediateThenReloads() async throws {
        runner.answers["ports"] = portsAnswer([serverObject(port: 3000, pid: 10)])
        await state.reload()
        runner.answers["stop 10 3000"] = try stopAnswer()
        runner.answers["ports"] = portsAnswer([])
        await state.stop(ServerId(pid: 10, port: 3000))
        #expect(runner.calls.map { $0.joined(separator: " ") } == ["ports", "stop 10 3000", "ports"])
        #expect(state.dialog == nil)
        #expect(state.notice == nil)
        #expect(state.stopping.isEmpty)
        #expect(state.placeholder == "No node server running")
    }

    @Test func aRowShowsStoppingWhileItsStopRuns() async throws {
        runner.answers["ports"] = portsAnswer([serverObject(port: 3000, pid: 10)])
        await state.reload()
        runner.answers["stop 10 3000"] = try stopAnswer()
        runner.delay = 50_000_000
        let task = Task { await state.stop(ServerId(pid: 10, port: 3000)) }
        try await Task.sleep(nanoseconds: 10_000_000)
        #expect(state.rows[0].stopLabel == "Stopping…")
        #expect(state.rows[0].isStopping)
        await task.value
    }

    @Test func aFailedStopFromTheListShowsTheMessageBelow() async {
        runner.answers["ports"] = portsAnswer([serverObject(port: 3000, pid: 10)])
        await state.reload()
        runner.answers["stop 10 3000"] = errorAnswer("not_listening", "Process 10 no longer listens on port 3000.")
        await state.stop(ServerId(pid: 10, port: 3000))
        #expect(state.notice == "Process 10 no longer listens on port 3000.")
        #expect(runner.calls.last == ["ports"])
    }

    @Test func aSearchShowsWhatListensOnThatPort() async {
        runner.answers["ports 5432"] = portsAnswer([serverObject(port: 5432, pid: 22, critical: "PostgreSQL")])
        state.query = " 5432 "
        await state.submitSearch()
        #expect(state.rows.map(\.port) == ["5432"])
        #expect(runner.calls == [["ports", "5432"]])
    }

    @Test func aSearchWithNothingListeningSaysSo() async {
        runner.answers["ports 8080"] = portsAnswer([])
        state.query = "8080"
        await state.submitSearch()
        #expect(state.placeholder == "Nothing listens on port 8080")
    }

    @Test func aPortHeldByAnInvisibleProcessIsExplained() async {
        runner.answers["ports 22"] = portsAnswer([], hidden: true)
        state.query = "22"
        await state.submitSearch()
        #expect(state.placeholder == "Port 22 is used by a process owl cannot see (system or another user).")
    }

    @Test(arguments: ["abc", "0", "65536", "-1", "30 00"])
    func anInvalidPortIsRefusedWithoutCallingOwl(text: String) async {
        state.query = text
        await state.submitSearch()
        #expect(state.placeholder == "Not a port: \(text)")
        #expect(runner.calls.isEmpty)
    }

    @Test func aDoubleEnterRunsOneSearch() async {
        runner.answers["ports 3000"] = portsAnswer([])
        runner.delay = 50_000_000
        state.query = "3000"
        async let first: Void = state.submitSearch()
        async let second: Void = state.submitSearch()
        _ = await (first, second)
        #expect(runner.calls.count == 1)
    }

    @Test func clearingTheFieldReturnsToTheList() async {
        runner.answers["ports 3000"] = portsAnswer([])
        runner.answers["ports"] = portsAnswer([serverObject(port: 3000, pid: 10)])
        state.query = "3000"
        await state.submitSearch()
        state.query = ""
        await state.queryChanged()
        #expect(state.search == nil)
        #expect(state.rows.count == 1)
    }

    @Test func escapeClearsTheSearch() async {
        runner.answers["ports 3000"] = portsAnswer([])
        runner.answers["ports"] = portsAnswer([])
        state.query = "3000"
        await state.submitSearch()
        await state.press(.escape)
        #expect(state.search == nil)
        #expect(state.query == "")
    }

    @Test func enterInTheServersTabSubmitsTheSearch() async {
        runner.answers["ports 3000"] = portsAnswer([])
        state.query = "3000"
        await state.press(.enter)
        #expect(runner.calls == [["ports", "3000"]])
    }

    @Test func stoppingASearchResultAlwaysAsksFirst() async throws {
        runner.answers["ports 3000"] = portsAnswer([serverObject(port: 3000, pid: 10)])
        state.query = "3000"
        await state.submitSearch()
        await state.stop(ServerId(pid: 10, port: 3000))
        #expect(state.dialog?.stage == .confirming)
        #expect(!runner.calls.contains(["stop", "10", "3000"]))
        let render = try #require(state.dialogRendering)
        #expect(render.title == "Stop npm run dev on port 3000?")
        #expect(render.frameTone == nil)
        #expect(render.confirmLabel == "Stop")
        #expect(render.cancelLabel == "Cancel")
    }

    @Test func aCriticalPortIsFramedInRedWithItsReason() async throws {
        runner.answers["ports 5432"] = portsAnswer([serverObject(port: 5432, pid: 22, critical: "PostgreSQL")])
        state.query = "5432"
        await state.submitSearch()
        await state.stop(ServerId(pid: 22, port: 5432))
        let render = try #require(state.dialogRendering)
        #expect(render.frameTone == .red)
        #expect(render.lines.contains([Segment(text: "Critical: PostgreSQL", tone: .red)]))
    }

    @Test func confirmingStopsThenSearchesAgain() async throws {
        runner.answers["ports 3000"] = portsAnswer([serverObject(port: 3000, pid: 10)])
        state.query = "3000"
        await state.submitSearch()
        await state.stop(ServerId(pid: 10, port: 3000))
        runner.answers["stop 10 3000"] = try stopAnswer()
        runner.answers["ports 3000"] = portsAnswer([])
        await state.press(.enter)
        #expect(state.dialog == nil)
        #expect(runner.calls.suffix(2).map { $0.joined(separator: " ") } == ["stop 10 3000", "ports 3000"])
        #expect(state.placeholder == "Nothing listens on port 3000")
    }

    @Test func aFailedConfirmedStopStaysInTheWindow() async {
        runner.answers["ports 3000"] = portsAnswer([serverObject(port: 3000, pid: 10)])
        state.query = "3000"
        await state.submitSearch()
        await state.stop(ServerId(pid: 10, port: 3000))
        runner.answers["stop 10 3000"] = errorAnswer("permission", "Permission denied: process 10 belongs to root.")
        await state.confirmStop()
        #expect(state.dialog?.stage == .failed("Permission denied: process 10 belongs to root."))
        #expect(state.dialogRendering?.lines.last == [Segment(text: "Permission denied: process 10 belongs to root.", tone: .red)])
    }

    @Test func escapeOrCancelClosesTheWindowWithoutStopping() async {
        runner.answers["ports 3000"] = portsAnswer([serverObject(port: 3000, pid: 10)])
        state.query = "3000"
        await state.submitSearch()
        await state.stop(ServerId(pid: 10, port: 3000))
        await state.press(.escape)
        #expect(state.dialog == nil)
        #expect(state.search != nil)
        #expect(runner.calls.count == 1)
    }

    @Test func whileStoppingTheWindowHasNoButtonAndCannotBeClosed() async throws {
        runner.answers["ports 3000"] = portsAnswer([serverObject(port: 3000, pid: 10)])
        state.query = "3000"
        await state.submitSearch()
        await state.stop(ServerId(pid: 10, port: 3000))
        runner.answers["stop 10 3000"] = try stopAnswer()
        runner.delay = 50_000_000
        let task = Task { await state.confirmStop() }
        try await Task.sleep(nanoseconds: 10_000_000)
        #expect(state.dialogRendering?.confirmLabel == nil)
        #expect(state.dialogRendering?.cancelLabel == nil)
        state.cancelStop()
        #expect(state.dialog != nil)
        await task.value
    }

    @Test func portParsing() {
        #expect(parsePort("3000") == 3000)
        #expect(parsePort(" 3000 ") == 3000)
        #expect(parsePort("1") == 1)
        #expect(parsePort("65535") == 65535)
        #expect(parsePort("0") == nil)
        #expect(parsePort("65536") == nil)
        #expect(parsePort("") == nil)
        #expect(parsePort("3e3") == nil)
    }
}
