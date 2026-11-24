// Sizes from the Android screen spec (section 1 and 1.1): touch targets and agent glyph sizes in dp.

/** The hit target of every control; the visual height of a control is CONTROL_HEIGHT. */
export const TOUCH_TARGET = 48;
export const CONTROL_HEIGHT = 44;
/** The multi-line pairing code field (screen M-PA-02). */
export const MULTILINE_FIELD_HEIGHT = 112;

export const GLYPH_SIZE = { small: 12, medium: 16, large: 20 } as const;
export type GlyphSize = (typeof GLYPH_SIZE)[keyof typeof GLYPH_SIZE];
