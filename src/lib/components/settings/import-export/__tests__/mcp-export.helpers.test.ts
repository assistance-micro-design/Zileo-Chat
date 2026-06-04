import { describe, expect, it } from 'vitest';
import { isLocalOrPrivateHttpServer, isLoopbackOrPrivateHost } from '../mcp-export.helpers';

describe('mcp-export helpers', () => {
	describe('isLoopbackOrPrivateHost', () => {
		it('flags loopback and localhost hosts', () => {
			expect(isLoopbackOrPrivateHost('localhost')).toBe(true);
			expect(isLoopbackOrPrivateHost('api.localhost')).toBe(true);
			expect(isLoopbackOrPrivateHost('server.local')).toBe(true);
			expect(isLoopbackOrPrivateHost('127.0.0.1')).toBe(true);
			expect(isLoopbackOrPrivateHost('127.5.5.5')).toBe(true);
			expect(isLoopbackOrPrivateHost('[::1]')).toBe(true);
			expect(isLoopbackOrPrivateHost('0.0.0.0')).toBe(true);
		});

		it('flags RFC1918 / CGNAT / link-local ranges', () => {
			expect(isLoopbackOrPrivateHost('10.0.0.1')).toBe(true);
			expect(isLoopbackOrPrivateHost('192.168.1.10')).toBe(true);
			expect(isLoopbackOrPrivateHost('172.16.0.1')).toBe(true);
			expect(isLoopbackOrPrivateHost('172.31.255.255')).toBe(true);
			expect(isLoopbackOrPrivateHost('169.254.169.254')).toBe(true);
			expect(isLoopbackOrPrivateHost('100.64.0.1')).toBe(true);
			expect(isLoopbackOrPrivateHost('fd00::1')).toBe(true);
			expect(isLoopbackOrPrivateHost('fe80::1')).toBe(true);
		});

		it('does not flag public hosts', () => {
			expect(isLoopbackOrPrivateHost('api.example.com')).toBe(false);
			expect(isLoopbackOrPrivateHost('8.8.8.8')).toBe(false);
			expect(isLoopbackOrPrivateHost('172.32.0.1')).toBe(false); // just outside RFC1918
			expect(isLoopbackOrPrivateHost('192.169.1.1')).toBe(false);
			expect(isLoopbackOrPrivateHost('100.128.0.1')).toBe(false); // outside CGNAT
		});
	});

	describe('isLocalOrPrivateHttpServer', () => {
		it('flags HTTP servers targeting local/LAN URLs', () => {
			expect(isLocalOrPrivateHttpServer('http', ['http://localhost:8080/'])).toBe(true);
			expect(isLocalOrPrivateHttpServer('http', ['http://192.168.1.5:3000/mcp'])).toBe(true);
			expect(isLocalOrPrivateHttpServer('HTTP', ['https://127.0.0.1/'])).toBe(true);
		});

		it('does not flag public HTTP servers', () => {
			expect(isLocalOrPrivateHttpServer('http', ['https://api.example.com/mcp'])).toBe(false);
		});

		it('does not flag non-HTTP deployment methods', () => {
			expect(isLocalOrPrivateHttpServer('docker', ['run', '-i', 'image:tag'])).toBe(false);
			expect(isLocalOrPrivateHttpServer('npx', ['some-pkg'])).toBe(false);
		});

		it('is safe on missing or malformed URLs', () => {
			expect(isLocalOrPrivateHttpServer('http', [])).toBe(false);
			expect(isLocalOrPrivateHttpServer('http', ['not a url'])).toBe(false);
		});
	});
});
