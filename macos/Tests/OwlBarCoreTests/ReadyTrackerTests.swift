import Foundation
import Testing
@testable import OwlBarCore

/// Copie d'une PR enregistrée avec un autre verdict de fusion, en repassant
/// par le JSON : les modèles n'ont pas d'initialiseur public.
func withVerdict(_ pr: PullRequest, _ verdict: MergeVerdict, number: Int? = nil) throws -> PullRequest {
    let encoder = JSONEncoder()
    encoder.keyEncodingStrategy = .convertToSnakeCase
    encoder.dateEncodingStrategy = .iso8601
    var object = try #require(
        JSONSerialization.jsonObject(with: try encoder.encode(EncodablePR(pr))) as? [String: Any]
    )
    object["merge_state"] = verdict.rawValue
    if let number { object["number"] = number }
    let envelope: [String: Any] = ["version": 1, "ok": true, "data": object]
    return try decodeOwl(PullRequest.self, from: JSONSerialization.data(withJSONObject: envelope)).get()
}

/// Miroir encodable d'une PR, pour ne pas rendre les modèles encodables
/// pour les seuls tests.
struct EncodablePR: Encodable {
    let repo: String, number: Int, title: String, author: String, url: String
    let isDraft: Bool, checks: String, review: String, mergeable: String, mergeState: String
    let baseRef: String, headRef: String, updatedAt: Date, mergeMethods: [String]
    let checkRuns: [[String: String?]]

    init(_ pr: PullRequest) {
        repo = pr.repo; number = pr.number; title = pr.title; author = pr.author; url = pr.url
        isDraft = pr.isDraft; checks = pr.checks.rawValue; review = pr.review.rawValue
        mergeable = pr.mergeable.rawValue; mergeState = pr.mergeState.rawValue
        baseRef = pr.baseRef; headRef = pr.headRef; updatedAt = pr.updatedAt
        mergeMethods = pr.mergeMethods.map(\.rawValue)
        checkRuns = pr.checkRuns.map { ["name": $0.name, "state": $0.state.rawValue, "url": $0.url] }
    }
}

struct ReadyTrackerTests {
    @Test func nothingIsAnnouncedOnTheFirstList() throws {
        var tracker = ReadyTracker()
        #expect(tracker.update([try withVerdict(recordedList()[0], .clean)]) == [])
    }

    @Test func aPullRequestBecomingReadyIsAnnouncedOnce() throws {
        let pr = try recordedList()[0]
        var tracker = ReadyTracker()
        _ = tracker.update([try withVerdict(pr, .blocked)])
        let ready = try withVerdict(pr, .clean)
        #expect(tracker.update([ready]).map(\.key) == [ready.key])
        #expect(tracker.update([ready]) == [])
    }

    @Test func anUnknownVerdictChangesNothing() throws {
        let pr = try recordedList()[0]
        var tracker = ReadyTracker()
        _ = tracker.update([try withVerdict(pr, .clean)])
        _ = tracker.update([try withVerdict(pr, .unknown)])
        #expect(tracker.update([try withVerdict(pr, .clean)]) == [])
    }

    @Test func blockedThenReadyAgainIsAnnouncedAgain() throws {
        let pr = try recordedList()[0]
        var tracker = ReadyTracker()
        _ = tracker.update([try withVerdict(pr, .clean)])
        _ = tracker.update([try withVerdict(pr, .blocked)])
        #expect(tracker.update([try withVerdict(pr, .clean)]).count == 1)
    }

    @Test func aPullRequestLeavingTheListIsForgotten() throws {
        let pr = try recordedList()[0]
        var tracker = ReadyTracker()
        _ = tracker.update([try withVerdict(pr, .clean)])
        _ = tracker.update([])
        #expect(tracker.update([try withVerdict(pr, .clean)]).count == 1)
    }
}
