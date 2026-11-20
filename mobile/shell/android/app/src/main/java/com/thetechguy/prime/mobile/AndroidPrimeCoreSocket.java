package com.thetechguy.prime.mobile;

import android.net.LocalSocket;
import android.net.LocalSocketAddress;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;

/**
 * Android init's /dev/socket/prime_mobile_core reserved-domain stream.
 * Does not use TCP, a world-writable app socket, shell commands or root.
 *
 * Availability depends on an actually installed Android-native Core service,
 * verified SELinux and socket permissions. No background daemon is started
 * by this HOME client.
 */
public final class AndroidPrimeCoreSocket implements PrimeCoreClient.Exchange {
    private static final String CORE_SOCKET = "prime_mobile_core";
    private static final int MAX_BYTES = 128 * 1024;
    private static final int READ_TIMEOUT_MS = 1000;

    @Override
    public byte[] request(byte[] request) throws IOException {
        if (request == null || request.length == 0 || request.length > 512) {
            throw new IOException("INVALID_READ_ONLY_REQUEST");
        }
        try (LocalSocket socket = new LocalSocket()) {
            socket.connect(new LocalSocketAddress(
                    CORE_SOCKET, LocalSocketAddress.Namespace.RESERVED));
            socket.setSoTimeout(READ_TIMEOUT_MS);
            socket.getOutputStream().write(request);
            socket.getOutputStream().flush();

            try (ByteArrayOutputStream output = new ByteArrayOutputStream()) {
                InputStream input = socket.getInputStream();
                byte[] buffer = new byte[4096];
                int n;
                while ((n = input.read(buffer)) != -1) {
                    if (output.size() + n > MAX_BYTES) {
                        throw new IOException("CORE_REPLY_TOO_LARGE");
                    }
                    output.write(buffer, 0, n);
                }
                return output.toByteArray();
            }
        }
    }
}
