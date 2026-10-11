import { describe, it, expect } from 'vitest';
import { escapeHtml, formatBytes } from '../src/index';

describe('escapeHtml', () => {
  it('should escape HTML special characters', () => {
    expect(escapeHtml('<div>test</div>')).toBe('&lt;div&gt;test&lt;/div&gt;');
    expect(escapeHtml('"quoted"')).toBe('&quot;quoted&quot;');
    expect(escapeHtml("it's")).toBe('it&apos;s');
  });

  it('should handle plain text', () => {
    expect(escapeHtml('Hello World')).toBe('Hello World');
  });

  it('should handle empty string', () => {
    expect(escapeHtml('')).toBe('');
  });
});

describe('formatBytes', () => {
  it('should format bytes correctly', () => {
    expect(formatBytes(0)).toBe('0 B');
    expect(formatBytes(1023)).toBe('1023 B');
    expect(formatBytes(1024)).toBe('1 KB');
    expect(formatBytes(1536)).toBe('1.5 KB');
    expect(formatBytes(1048576)).toBe('1 MB');
  });
});
