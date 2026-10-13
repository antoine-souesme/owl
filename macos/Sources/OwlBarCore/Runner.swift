// Recherche et lancement de `owl`. C'est le seul endroit de l'app qui lance
// un processus. `owl` voyage dans l'app elle-même, à côté de son exécutable :
// rien n'a besoin d'être installé dans le terminal, et l'app parle toujours à
// la version de `owl` construite avec elle. Une app ouverte depuis le Finder
// ne voit pas le PATH du terminal : les dossiers habituels de Homebrew sont
// transmis à `owl` pour qu'il trouve `gh`.
import Foundation

public protocol OwlRunning: Sendable {
    /// Lance `owl` avec ces arguments et rend sa sortie standard brute.
    func run(_ arguments: [String]) async -> Result<Data, OwlFailure>
}

/// Dossiers où chercher `gh`, placés en tête du PATH de `owl`.
let ghDirectories = ["/opt/homebrew/bin", "/usr/local/bin"]

/// `owl` rangé à côté de l'exécutable de l'app. Un `owl` installé ailleurs
/// est ignoré : il pourrait ne pas parler le même format.
public func locateOwl(executableDirectory: String?, isExecutable: (String) -> Bool) -> String? {
    guard let executableDirectory else { return nil }
    let path = executableDirectory + "/owl"
    return isExecutable(path) ? path : nil
}

public func childPath(inherited: String?) -> String {
    (ghDirectories + (inherited.map { [$0] } ?? [])).joined(separator: ":")
}

public struct ProcessRunner: OwlRunning {
    public init() {}

    public func run(_ arguments: [String]) async -> Result<Data, OwlFailure> {
        let executableDirectory = Bundle.main.executableURL?.deletingLastPathComponent().path
        guard let path = locateOwl(
            executableDirectory: executableDirectory,
            isExecutable: FileManager.default.isExecutableFile(atPath:)
        ) else {
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
                environment["PATH"] = childPath(inherited: environment["PATH"])
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
