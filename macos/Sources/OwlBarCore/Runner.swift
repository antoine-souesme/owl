// Recherche et lancement de `owl`. C'est le seul endroit de l'app qui lance
// un processus. Une app ouverte depuis le Finder ne voit pas le PATH du
// terminal : les dossiers d'installation habituels sont parcourus à la main,
// puis transmis à `owl` pour qu'il trouve `gh` à son tour.
import Foundation

public protocol OwlRunning: Sendable {
    /// Lance `owl` avec ces arguments et rend sa sortie standard brute.
    func run(_ arguments: [String]) async -> Result<Data, OwlFailure>
}

/// Dossiers parcourus, dans l'ordre. `~` désigne le dossier personnel.
let searchDirectories = ["~/.cargo/bin", "/opt/homebrew/bin", "/usr/local/bin"]

private func expanded(_ directory: String, home: String) -> String {
    directory.hasPrefix("~") ? home + directory.dropFirst() : directory
}

public func locateOwl(home: String, isExecutable: (String) -> Bool) -> String? {
    searchDirectories
        .map { expanded($0, home: home) + "/owl" }
        .first(where: isExecutable)
}

public func childPath(home: String, inherited: String?) -> String {
    let own = searchDirectories.map { expanded($0, home: home) }
    let rest = inherited.map { [$0] } ?? []
    return (own + rest).joined(separator: ":")
}

public struct ProcessRunner: OwlRunning {
    public init() {}

    public func run(_ arguments: [String]) async -> Result<Data, OwlFailure> {
        let home = NSHomeDirectory()
        guard let path = locateOwl(home: home, isExecutable: FileManager.default.isExecutableFile(atPath:)) else {
            return .failure(.notFound)
        }
        // Lecture bloquante hors du fil principal : le panneau reste
        // réactif pendant que `owl` interroge GitHub.
        return await withCheckedContinuation { continuation in
            DispatchQueue.global(qos: .userInitiated).async {
                let process = Process()
                process.executableURL = URL(fileURLWithPath: path)
                process.arguments = arguments
                var environment = ProcessInfo.processInfo.environment
                environment["PATH"] = childPath(home: home, inherited: environment["PATH"])
                process.environment = environment
                let output = Pipe()
                process.standardOutput = output
                // La sortie d'erreur ne porte que l'aide d'usage : rien à
                // montrer, l'enveloppe absente suffit à signaler le problème.
                process.standardError = FileHandle.nullDevice
                do {
                    try process.run()
                } catch {
                    continuation.resume(returning: .failure(.notFound))
                    return
                }
                let data = output.fileHandleForReading.readDataToEndOfFile()
                process.waitUntilExit()
                continuation.resume(returning: .success(data))
            }
        }
    }
}
