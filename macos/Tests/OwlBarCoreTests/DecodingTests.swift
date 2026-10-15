import Foundation
import Testing
@testable import OwlBarCore

func fixture(_ name: String) throws -> Data {
    let url = try #require(
        Bundle.module.url(forResource: name, withExtension: "json", subdirectory: "Fixtures")
    )
    return try Data(contentsOf: url)
}

struct DecodingTests {
    @Test func theRecordedListIsReadInFull() throws {
        let list = try decodeOwl(ListData.self, from: fixture("list")).get()
        #expect(list.pullRequests.count == 5)
        #expect(list.preferredMergeMethod == .squash)
        #expect(list.refreshInterval == 60)
        let first = list.pullRequests[0]
        #expect(first.key == PrKey(repo: "moi/owl", number: 42))
        #expect(first.checks == .success)
        #expect(first.review == .approved)
        #expect(first.mergeState == .clean)
        #expect(first.mergeMethods == [.squash, .rebase])
        #expect(first.baseRef == "develop")
    }

    @Test func theRecordedDetailIsReadInFull() throws {
        let detail = try decodeOwl(PrDetail.self, from: fixture("detail")).get()
        #expect(detail.summary.number == 42)
        #expect(detail.additions == 214)
        #expect(!detail.checks.isEmpty)
    }

    @Test func aMergeSuccessIsAnEmptyObject() throws {
        let data = Data(#"{"version":1,"ok":true,"data":{}}"#.utf8)
        #expect(throws: Never.self) { try decodeOwl(MergeData.self, from: data).get() }
    }

    @Test func anErrorEnvelopeGivesOwlsMessageVerbatim() {
        let data = Data(#"{"version":1,"ok":false,"error":{"kind":"api","message":"Base branch was modified."}}"#.utf8)
        let result = decodeOwl(MergeData.self, from: data)
        #expect(result == .failure(OwlFailure(kind: "api", message: "Base branch was modified.", resetAt: nil)))
    }

    @Test func aRateLimitCarriesItsResetTime() throws {
        let data = Data(#"{"version":1,"ok":false,"error":{"kind":"rate_limited","message":"Rate limit reached.","reset_at":"2026-10-13T14:32:00Z"}}"#.utf8)
        guard case .failure(let failure) = decodeOwl(ListData.self, from: data) else {
            Issue.record("échec attendu")
            return
        }
        #expect(failure.resetAt == ISO8601DateFormatter().date(from: "2026-10-13T14:32:00Z"))
    }

    @Test func anotherFormatVersionIsRefused() {
        let data = Data(#"{"version":2,"ok":true,"data":{}}"#.utf8)
        #expect(decodeOwl(MergeData.self, from: data) == .failure(.unsupported))
    }

    @Test(arguments: ["", "Usage: owl [list | …]", "{\"ok\":true}"])
    func anythingButJsonIsUnreadable(text: String) {
        #expect(decodeOwl(MergeData.self, from: Data(text.utf8)) == .failure(.unreadable))
    }

    @Test func theFixedMessagesAreExact() {
        #expect(OwlFailure.unsupported.message == "This owl version is not supported by the menu bar app. Update both.")
        #expect(OwlFailure.unreadable.message == "Unreadable answer from owl.")
    }

    @Test func theRecordedServersAreReadInFull() throws {
        let ports = try decodeOwl(PortsData.self, from: fixture("ports")).get()
        #expect(ports.hidden == false)
        #expect(ports.servers.map(\.port) == [3000, 5173])
        let next = ports.servers[0]
        #expect(next.id == ServerId(pid: 35860, port: 3000))
        #expect(next.project == "erp-menage")
        #expect(next.directory == "/Users/antoinesouesme/dev/projets/gkfa/erp-menage")
        #expect(next.command == "npm run dev")
        #expect(next.startedAt == ISO8601DateFormatter().date(from: "2026-10-14T20:31:47Z"))
        #expect(next.critical == nil)
    }

    @Test func aCriticalServerWithoutProjectIsRead() throws {
        let data = Data(#"{"version":1,"ok":true,"data":{"servers":[{"port":7000,"pid":1055,"project":null,"directory":"/","command":"ControlCenter","started_at":"2026-10-01T09:00:00Z","user":"moi","critical":{"reason":"AirPlay Receiver"}}],"hidden":false}}"#.utf8)
        let server = try decodeOwl(PortsData.self, from: data).get().servers[0]
        #expect(server.project == nil)
        #expect(server.critical == Critical(reason: "AirPlay Receiver"))
    }

    @Test func theRecordedStopIsRead() throws {
        let stop = try decodeOwl(StopData.self, from: fixture("stop")).get()
        #expect(stop.stopped == [35815, 35834, 35860])
        #expect(stop.forced == false)
    }
}
