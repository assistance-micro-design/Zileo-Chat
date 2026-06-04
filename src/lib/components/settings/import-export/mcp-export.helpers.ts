/**
 * Helpers for the MCP-server export flow.
 *
 * An HTTP MCP server pointing at a loopback or private/LAN address works at
 * runtime but is deliberately rejected when re-imported (SSRF defense — an
 * import file may come from an untrusted third party). We surface that up
 * front at export time so the user knows the server will have to be re-created
 * by hand on the target machine.
 *
 * This classification is best-effort and advisory; the authoritative screen
 * lives in the Rust backend (`mcp/ssrf.rs`).
 */

/**
 * Returns true when `host` is a loopback, private (RFC1918 / CGNAT / ULA) or
 * link-local address — i.e. one the import-time SSRF screen refuses.
 *
 * @param host - a URL hostname (IPv6 brackets are tolerated)
 */
export function isLoopbackOrPrivateHost(host: string): boolean {
	const h = host.toLowerCase().replace(/^\[/, '').replace(/\]$/, '');
	if (h.length === 0) return false;

	// Hostnames
	if (h === 'localhost' || h.endsWith('.localhost') || h.endsWith('.local')) return true;

	// IPv6 loopback / unspecified / link-local / ULA
	if (h === '::1' || h === '::') return true;
	if (h.startsWith('fe80:')) return true; // link-local
	if (h.startsWith('fc') || h.startsWith('fd')) return true; // ULA fc00::/7

	// IPv4 dotted-quad
	const m = h.match(/^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})$/);
	if (m) {
		const a = Number(m[1]);
		const b = Number(m[2]);
		if (a === 0 || a === 127) return true; // unspecified / loopback
		if (a === 10) return true; // RFC1918
		if (a === 192 && b === 168) return true; // RFC1918
		if (a === 172 && b >= 16 && b <= 31) return true; // RFC1918
		if (a === 169 && b === 254) return true; // link-local / cloud metadata
		if (a === 100 && b >= 64 && b <= 127) return true; // CGNAT
	}

	return false;
}

/**
 * Returns true when an MCP server will NOT survive a re-import because it is an
 * HTTP server whose URL targets a loopback/private/LAN address.
 *
 * Non-HTTP servers (docker/npx/uvx) and public HTTP endpoints return false.
 *
 * @param command - the deployment method (`'http'`, `'docker'`, …)
 * @param args - command arguments; `args[0]` is the URL for HTTP servers
 */
export function isLocalOrPrivateHttpServer(command: string, args: string[]): boolean {
	if (command.toLowerCase() !== 'http') return false;
	const url = args[0];
	if (!url) return false;
	let host: string;
	try {
		host = new URL(url).hostname;
	} catch {
		return false;
	}
	return isLoopbackOrPrivateHost(host);
}
