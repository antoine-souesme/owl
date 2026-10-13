// Lecture de l'enveloppe écrite par `owl`. Toute réponse qui n'en est pas
// une devient une erreur affichable : l'app ne plante jamais sur une sortie
// inattendue.
import Foundation

/// Seule version du format que l'app sait lire.
public let supportedFormatVersion = 1

/// Erreur affichable. `message` est montré tel quel ; `kind` sert à réagir
/// sans lire le texte.
public struct OwlFailure: Error, Equatable, Decodable, Sendable {
    public let kind: String
    public let message: String
    public let resetAt: Date?

    public init(kind: String, message: String, resetAt: Date?) {
        self.kind = kind
        self.message = message
        self.resetAt = resetAt
    }

    public static let notFound = OwlFailure(
        kind: "owl_not_found",
        message: "owl not found. Install it with `cargo install --path .`",
        resetAt: nil
    )
    public static let unsupported = OwlFailure(
        kind: "unsupported_version",
        message: "This owl version is not supported by the menu bar app. Update both.",
        resetAt: nil
    )
    public static let unreadable = OwlFailure(
        kind: "unreadable",
        message: "Unreadable answer from owl.",
        resetAt: nil
    )
}

private struct Head: Decodable {
    let version: Int
}

private struct Body<T: Decodable>: Decodable {
    let ok: Bool
    let data: T?
    let error: OwlFailure?
}

public func decodeOwl<T: Decodable>(_ type: T.Type, from data: Data) -> Result<T, OwlFailure> {
    let decoder = JSONDecoder()
    decoder.keyDecodingStrategy = .convertFromSnakeCase
    decoder.dateDecodingStrategy = .iso8601

    // La version est lue seule d'abord : un format inconnu doit être refusé
    // même si le reste ne se lit pas.
    guard let head = try? decoder.decode(Head.self, from: data) else {
        return .failure(.unreadable)
    }
    guard head.version == supportedFormatVersion else {
        return .failure(.unsupported)
    }
    guard let body = try? decoder.decode(Body<T>.self, from: data) else {
        return .failure(.unreadable)
    }
    if body.ok, let value = body.data {
        return .success(value)
    }
    return .failure(body.error ?? .unreadable)
}
