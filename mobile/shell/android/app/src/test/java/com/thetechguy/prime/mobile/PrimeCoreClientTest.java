package com.thetechguy.prime.mobile;

import static org.junit.Assert.*;

import org.junit.Test;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;

public final class PrimeCoreClientTest {
    private static byte[] reply(int status, String json) {
        byte[] bytes = json.getBytes(StandardCharsets.UTF_8);
        String head = "HTTP/1.1 " + status + " OK\r\n"
                + "content-type: application/json\r\n"
                + "content-length: " + bytes.length + "\r\n\r\n";
        byte[] prefix = head.getBytes(StandardCharsets.UTF_8);
        byte[] packet = new byte[prefix.length + bytes.length];
        System.arraycopy(prefix, 0, packet, 0, prefix.length);
        System.arraycopy(bytes, 0, packet, prefix.length, bytes.length);
        return packet;
    }

    private static final String VERSIONS = "{\"interface\":\"prime.capability.v1\","
            + "\"supported_versions\":[\"1.0\"]}";
    private static final String CAPABILITIES = "{\"interface\":\"prime.capability.v1\","
            + "\"interface_version\":\"1.0\","
            + "\"capabilities\":[{\"capability_id\":\"prime.hardware.read\"},"
            + "{\"capability_id\":\"prime.host.identity\"}]}";

    private static class FakeExchange implements PrimeCoreClient.Exchange {
        final List<byte[]> requests = new ArrayList<>();
        final List<byte[]> responses = new ArrayList<>();
        @Override public byte[] request(byte[] request) throws IOException {
            requests.add(request);
            if (responses.size() < requests.size()) throw new IOException("not connected");
            return responses.get(requests.size() - 1);
        }
        FakeExchange returns(byte[] data) { responses.add(data); return this; }
    }

    @Test public void negotiatedReadOnlyCoreShowsUnqualifiedStatus() {
        FakeExchange backend = new FakeExchange().returns(reply(200, VERSIONS))
                .returns(reply(200, CAPABILITIES));
        PrimeCoreClient.Snapshot s = PrimeCoreClient.query(backend);
        assertEquals(PrimeCoreClient.State.RESPONDING_UNQUALIFIED, s.state);
        assertEquals(2, s.capabilities);
        assertEquals("Core API 2 capabilities (unverified)", s.label());
        assertEquals(2, backend.requests.size());
        String one = new String(backend.requests.get(0), StandardCharsets.US_ASCII);
        String two = new String(backend.requests.get(1), StandardCharsets.US_ASCII);
        assertTrue(one.startsWith("GET /v1/versions HTTP/1.1"));
        assertTrue(two.startsWith("GET /v1/capabilities HTTP/1.1"));
        assertFalse(one.contains("Prime-Interface-Accept"));
        assertTrue(two.contains("Prime-Interface-Accept: 1.0\r\n"));
        assertFalse(one.contains("POST "));
        assertFalse(two.contains("POST "));
    }

    @Test public void offlineNoSocketIsTruthfullyUnavailable() {
        PrimeCoreClient.Snapshot s = PrimeCoreClient.query(request -> {
            throw new IOException("socket not available");
        });
        assertEquals(PrimeCoreClient.State.UNAVAILABLE, s.state);
        assertEquals("Core unavailable", s.label());
        assertEquals(0, s.capabilities);
    }

    @Test public void incompatibleCoreVersionFailsClosed() {
        FakeExchange backend = new FakeExchange().returns(reply(200,
                "{\"interface\":\"prime.capability.v1\",\"supported_versions\":[\"9.0\"]}"));
        PrimeCoreClient.Snapshot s = PrimeCoreClient.query(backend);
        assertEquals(PrimeCoreClient.State.INCOMPATIBLE, s.state);
        assertEquals(1, backend.requests.size());
    }

    @Test public void invalidNegotiationStatusDoesNotGetPromoted() {
        FakeExchange backend = new FakeExchange().returns(reply(200, VERSIONS))
                .returns(reply(409, "{\"error\":\"version rejected\"}"));
        assertEquals(PrimeCoreClient.State.INCOMPATIBLE,
                PrimeCoreClient.query(backend).state);
    }

    @Test public void zeroCapabilitiesIsNotCoreReady() {
        FakeExchange backend = new FakeExchange().returns(reply(200, VERSIONS))
                .returns(reply(200, "{\"interface\":\"prime.capability.v1\","
                        + "\"interface_version\":\"1.0\",\"capabilities\":[]}"));
        assertEquals(PrimeCoreClient.State.UNAVAILABLE,
                PrimeCoreClient.query(backend).state);
    }

    @Test public void invalidCapabilityOrSchemaFails() {
        FakeExchange backend = new FakeExchange().returns(reply(200, VERSIONS))
                .returns(reply(200, CAPABILITIES.replace("prime.hardware.read", "invalid id")));
        assertEquals(PrimeCoreClient.State.UNAVAILABLE,
                PrimeCoreClient.query(backend).state);
        FakeExchange badSchema = new FakeExchange().returns(reply(200, VERSIONS))
                .returns(reply(200, CAPABILITIES.replace("\"interface_version\":\"1.0\"",
                        "\"interface_version\":\"9.0\"")));
        assertEquals(PrimeCoreClient.State.INCOMPATIBLE,
                PrimeCoreClient.query(badSchema).state);
    }

    @Test public void malformedOrOversizedResponsesFailClosed() {
        FakeExchange empty = new FakeExchange().returns(new byte[0]);
        assertEquals(PrimeCoreClient.State.UNAVAILABLE,
                PrimeCoreClient.query(empty).state);
        FakeExchange giant = new FakeExchange().returns(new byte[128 * 1024 + 1]);
        assertEquals(PrimeCoreClient.State.UNAVAILABLE,
                PrimeCoreClient.query(giant).state);
    }

    @Test public void duplicateContentLengthIsRejected() {
        String valid = new String(reply(200, VERSIONS), StandardCharsets.UTF_8);
        String altered = valid.replace("content-length:", "Content-Length: 40\r\ncontent-length:");
        FakeExchange backend = new FakeExchange()
                .returns(altered.getBytes(StandardCharsets.UTF_8));
        assertEquals(PrimeCoreClient.State.UNAVAILABLE,
                PrimeCoreClient.query(backend).state);
    }

    @Test public void nonJsonErrorBodyIsNeverPresentedAsCore() {
        FakeExchange backend = new FakeExchange().returns(reply(200,
                "not JSON at all"));
        assertEquals(PrimeCoreClient.State.UNAVAILABLE,
                PrimeCoreClient.query(backend).state);
    }
}
