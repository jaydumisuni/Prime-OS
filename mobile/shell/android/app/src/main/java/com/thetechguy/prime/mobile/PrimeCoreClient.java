package com.thetechguy.prime.mobile;

import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.util.Locale;

/**
 * A restricted read-only client for the existing Prime Core HTTP-over-Unix
 * interface. No Android systemd/bootc assumptions, mutation methods, TCP
 * fallback, identity promotion, or optional AI dependency.
 *
 * A responding socket is NOT a bootc-qualified or physically proven Core.
 */
public final class PrimeCoreClient {
    public interface Exchange {
        byte[] request(byte[] httpRequest) throws IOException;
    }

    public enum State {
        UNAVAILABLE,
        INCOMPATIBLE,
        RESPONDING_UNQUALIFIED
    }

    public static final class Snapshot {
        public final State state;
        public final int capabilities;
        public final String limitation;

        private Snapshot(State state, int count, String limitation) {
            this.state = state;
            this.capabilities = count;
            this.limitation = limitation;
        }

        public String label() {
            switch (state) {
                case RESPONDING_UNQUALIFIED:
                    return "Core API " + capabilities + " capabilities (unverified)";
                case INCOMPATIBLE:
                    return "Core API incompatible";
                default:
                    return "Core unavailable";
            }
        }
    }

    private static final String INTERFACE = "prime.capability.v1";
    private static final String VERSION = "1.0";
    private static final int MAX_RESPONSE = 128 * 1024;
    private static final int MAX_CAPABILITIES = 128;

    private PrimeCoreClient() {}

    public static Snapshot query(Exchange transport) {
        if (transport == null) return unavailable("NO_TRANSPORT");
        try {
            Reply versions = parse(transport.request(http("/v1/versions", false)));
            if (versions.status != 200) return unavailable("VERSION_ENDPOINT_UNAVAILABLE");
            JSONObject info = new JSONObject(versions.body);
            JSONArray supported = info.getJSONArray("supported_versions");
            boolean advertised = false;
            for (int i = 0; i < supported.length(); i++) {
                if (VERSION.equals(supported.getString(i))) advertised = true;
            }
            if (!INTERFACE.equals(info.getString("interface")) || !advertised) {
                return new Snapshot(State.INCOMPATIBLE, 0, "INTERFACE_VERSION_NOT_SUPPORTED");
            }

            Reply capabilities = parse(transport.request(http("/v1/capabilities", true)));
            if (capabilities.status == 400 || capabilities.status == 409) {
                return new Snapshot(State.INCOMPATIBLE, 0, "CAPABILITY_NEGOTIATION_REJECTED");
            }
            if (capabilities.status != 200) return unavailable("CAPABILITY_ENDPOINT_UNAVAILABLE");

            JSONObject body = new JSONObject(capabilities.body);
            if (!INTERFACE.equals(body.getString("interface")) ||
                    !VERSION.equals(body.getString("interface_version"))) {
                return new Snapshot(State.INCOMPATIBLE, 0, "CAPABILITY_SCHEMA_MISMATCH");
            }
            JSONArray items = body.getJSONArray("capabilities");
            if (items.length() == 0 || items.length() > MAX_CAPABILITIES) {
                return unavailable("CAPABILITY_COUNT_OUT_OF_RANGE");
            }
            for (int i = 0; i < items.length(); i++) {
                String id = items.getJSONObject(i).getString("capability_id");
                if (id.isEmpty() || id.length() > 128 || !id.matches("[a-z0-9._-]+")) {
                    return unavailable("INVALID_CAPABILITY_ID");
                }
            }
            // No claimed device trust, bootc generation admission, or mutation access.
            return new Snapshot(State.RESPONDING_UNQUALIFIED, items.length(),
                    "CORE_TRANSPORT_ONLY_NOT_DEVICE_OR_BOOT_ADMISSION");
        } catch (IOException | JSONException | IllegalArgumentException error) {
            // Do not return socket path, private peer details or JSON text to UI.
            return unavailable("CORE_READ_UNAVAILABLE");
        }
    }

    private static Snapshot unavailable(String reason) {
        return new Snapshot(State.UNAVAILABLE, 0, reason);
    }

    private static byte[] http(String resource, boolean negotiate) {
        String text = "GET " + resource + " HTTP/1.1\r\n" +
                "Host: localhost\r\n" +
                (negotiate ? "Prime-Interface-Accept: " + VERSION + "\r\n" : "") +
                "Connection: close\r\n\r\n";
        return text.getBytes(StandardCharsets.US_ASCII);
    }

    private static final class Reply {
        final int status;
        final String body;
        Reply(int status, String body) { this.status = status; this.body = body; }
    }

    static Reply parse(byte[] response) throws IOException {
        if (response == null || response.length < 17 || response.length > MAX_RESPONSE) {
            throw new IOException("BOUNDED_HTTP_RESPONSE_REQUIRED");
        }
        String raw = new String(response, StandardCharsets.UTF_8);
        int split = raw.indexOf("\r\n\r\n");
        if (split < 0 || split > 4096) throw new IOException("HTTP_HEADER_INVALID");
        String header = raw.substring(0, split);
        String[] lines = header.split("\r\n", -1);
        if (!lines[0].matches("HTTP/1\\.[01] [1-5][0-9][0-9](?: .*)?")) {
            throw new IOException("HTTP_STATUS_INVALID");
        }
        int code = Integer.parseInt(lines[0].substring(9, 12));
        boolean lengthSet = false;
        int contentLength = -1;
        for (int i = 1; i < lines.length; i++) {
            int separator = lines[i].indexOf(':');
            if (separator < 1) throw new IOException("HTTP_HEADER_FIELD_INVALID");
            String name = lines[i].substring(0, separator).trim().toLowerCase(Locale.ROOT);
            String value = lines[i].substring(separator + 1).trim();
            if ("transfer-encoding".equals(name)) {
                // Fail closed; no chunked parsing shortcuts or partial JSON reads.
                throw new IOException("CHUNKED_ENCODING_NOT_SUPPORTED");
            }
            if ("content-length".equals(name)) {
                if (lengthSet || !value.matches("[0-9]{1,6}"))
                    throw new IOException("DUPLICATE_OR_INVALID_CONTENT_LENGTH");
                lengthSet = true;
                contentLength = Integer.parseInt(value);
            }
        }
        if (!lengthSet || contentLength < 2 || contentLength > MAX_RESPONSE - split - 4) {
            throw new IOException("MISSING_OR_EXCESSIVE_CONTENT_LENGTH");
        }
        byte[] body = raw.substring(split + 4).getBytes(StandardCharsets.UTF_8);
        if (body.length != contentLength) throw new IOException("BODY_LENGTH_MISMATCH");
        return new Reply(code, new String(body, StandardCharsets.UTF_8));
    }
}
