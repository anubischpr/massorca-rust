const HOST_NAME_PATTERN = /^[a-z0-9][a-z0-9_-]{0,31}$/;

/** The rule of the Rust host store: 1 to 32 of a-z, 0-9, - and _, starting with a letter or digit, not "local". */
export function isValidHostName(name: string): boolean {
  return HOST_NAME_PATTERN.test(name) && name !== 'local';
}

/** The first label of the desktop's own name, cleaned to the rule above; empty when nothing usable is left. */
export function suggestHostName(server: string): string {
  const label = server.trim().toLowerCase().split('.')[0] ?? '';
  const cleaned = label.replace(/[^a-z0-9_-]+/g, '-').replace(/^-+/, '').slice(0, 32);
  return isValidHostName(cleaned) ? cleaned : '';
}
