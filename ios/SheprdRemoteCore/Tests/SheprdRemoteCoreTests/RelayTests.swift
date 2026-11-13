import CryptoKit
import Foundation
import Testing
@testable import SheprdRemoteCore

/// `sheprd-relay`'s tests write these vectors (`tests/ios_vectors.rs`).
struct RelayVectorTests {
    struct Endpoint: Decodable {
        let text: String
        let proof_name: String
        let dial: String
    }

    struct Dial: Decodable {
        let endpoints: [Endpoint]
        let proof_name: String
        let nonce: String
        let route: String
        let route_text: String
        let signed: String
    }

    static func load() throws -> Dial {
        let url = try #require(Bundle.module.url(forResource: "Fixtures/relay/dial", withExtension: "json"))
        return try JSONDecoder().decode(Dial.self, from: Data(contentsOf: url))
    }

    @Test func endpointsReadAsTheRelayCrateReadsThem() throws {
        let vector = try Self.load()
        for expected in vector.endpoints {
            let endpoint = try #require(RelayEndpoint(expected.text), "\(expected.text)")
            #expect(endpoint.proofName == expected.proof_name, "\(expected.text)")
            #expect(endpoint.dial(route: Data(repeating: 7, count: 32))?.absoluteString == expected.dial, "\(expected.text)")
        }
        #expect(RelayEndpoint("http://relay.example.org") == nil)
        #expect(RelayEndpoint("ftp://relay.example.org") == nil)
        #expect(RelayEndpoint("https://user@relay.example.org") == nil)
    }

    @Test func aDialerSignsTheBytesTheRelayChecks() throws {
        let vector = try Self.load()
        let nonce = try TunnelVectorTests.hex(vector.nonce)
        let route = try TunnelVectorTests.hex(vector.route)
        let signed = RelayProof.dialSigned(name: vector.proof_name, nonce: nonce, route: route)
        #expect(signed == (try TunnelVectorTests.hex(vector.signed)))
        #expect(Base64URL.encode(route) == vector.route_text)
        let key = P256.Signing.PrivateKey()
        let hello = try RelayProof.hello(key: key, signed: signed)
        let keyText = try #require(hello["key"])
        let signatureText = try #require(hello["signature"])
        let keyBytes = try #require(Base64URL.decode(keyText))
        let signatureBytes = try #require(Base64URL.decode(signatureText))
        #expect(keyBytes.count == 33)
        let publicKey = try P256.Signing.PublicKey(compressedRepresentation: keyBytes)
        let signature = try P256.Signing.ECDSASignature(rawRepresentation: signatureBytes)
        #expect(publicKey.isValidSignature(signature, for: signed))
    }

    @Test func anEndpointRoundTripsThroughJSON() throws {
        let endpoint = try #require(RelayEndpoint("relay.example.org:8443/sheprd/"))
        let data = try JSONEncoder().encode(endpoint)
        #expect(try JSONDecoder().decode(RelayEndpoint.self, from: data) == endpoint)
        #expect(endpoint.description == "https://relay.example.org:8443/sheprd")
    }
}
