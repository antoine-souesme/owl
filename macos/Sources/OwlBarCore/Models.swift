// Types lus dans le JSON de `owl`. Ils suivent le contrat de
// `src/cli/output.rs` ; les noms en snake_case sont convertis à la lecture.
import Foundation

public struct PrKey: Hashable, Sendable {
    public let repo: String
    public let number: Int

    public init(repo: String, number: Int) {
        self.repo = repo
        self.number = number
    }
}

public enum ChecksState: String, Decodable, Sendable {
    case success, failure, pending, none
}

public enum ReviewState: String, Decodable, Sendable {
    case approved
    case changesRequested = "changes_requested"
    case reviewRequired = "review_required"
    case none
}

public enum MergeableState: String, Decodable, Sendable {
    case mergeable, conflicting, unknown
}

/// Verdict de fusion de GitHub. `clean` : plus rien ne bloque.
public enum MergeVerdict: String, Decodable, Sendable {
    case clean, blocked, unknown
}

/// Dans l'ordre d'affichage : merge, squash, rebase.
public enum MergeMethod: String, Decodable, Sendable, CaseIterable {
    case merge, squash, rebase
}

public struct CheckRun: Decodable, Equatable, Sendable {
    public let name: String
    public let state: ChecksState
    public let url: String?
}

public struct PullRequest: Decodable, Equatable, Sendable, Identifiable {
    public let repo: String
    public let number: Int
    public let title: String
    public let author: String
    public let url: String
    public let isDraft: Bool
    public let checks: ChecksState
    public let review: ReviewState
    public let mergeable: MergeableState
    public let mergeState: MergeVerdict
    public let baseRef: String
    public let headRef: String
    public let updatedAt: Date
    public let mergeMethods: [MergeMethod]
    public let checkRuns: [CheckRun]

    public var key: PrKey { PrKey(repo: repo, number: number) }
    public var id: PrKey { key }
}

public struct RateLimit: Decodable, Equatable, Sendable {
    public let remaining: Int
    public let resetAt: Date
}

public struct ListData: Decodable, Equatable, Sendable {
    public let pullRequests: [PullRequest]
    public let preferredMergeMethod: MergeMethod
    public let refreshInterval: Int
    public let rateLimit: RateLimit?
}

public struct Review: Decodable, Equatable, Sendable {
    public let author: String
    public let state: ReviewState
    public let body: String
    public let submittedAt: Date
}

public struct Comment: Decodable, Equatable, Sendable {
    public let author: String
    public let body: String
    public let createdAt: Date
}

public struct ChangedFile: Decodable, Equatable, Sendable {
    public let path: String
    public let additions: Int
    public let deletions: Int
}

public struct PrDetail: Decodable, Equatable, Sendable {
    public let summary: PullRequest
    public let body: String
    public let checks: [CheckRun]
    public let reviews: [Review]
    public let comments: [Comment]
    public let files: [ChangedFile]
    public let additions: Int
    public let deletions: Int
}

/// Réponse d'une fusion réussie : un objet vide.
public struct MergeData: Decodable, Equatable, Sendable {}
