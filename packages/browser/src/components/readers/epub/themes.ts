import { SupportedFont } from '@stump/graphql'

export interface EpubTheme {
	[tag: string]: object
}

// TODO: I think we should use a CSS-in-JS library for this? This way, I can do things like:
// blockquote: {p: {color: '...'}}

// Note: Not React CSS, has to be true CSS fields. E.g. font-size not fontSize.
const createEpubTheme = (
	foreground: string,
	background: string,
	mutedForeground: string,
	linkForeground: string,
): EpubTheme => ({
	'body, body *': {
		color: `${foreground} !important`,
		'background-color': 'transparent !important',
	},
	'html, body': { 'background-color': `${background} !important` },
	p: { 'font-size': 'unset' },
	'blockquote, blockquote *': { color: `${mutedForeground} !important` },
	'a, a *': {
		color: `${linkForeground} !important`,
		'text-decoration': 'underline !important',
	},
})

export const EPUB_THEME_COLORS = {
	light: {
		foreground: '#000000',
		background: '#FFFFFF',
		mutedForeground: '#4A4A4A',
		linkForeground: '#005A9C',
	},
	dark: {
		foreground: '#E8EDF4',
		background: '#161719',
		mutedForeground: '#A8ACB0',
		linkForeground: '#4299E1',
	},
} as const

const light = EPUB_THEME_COLORS.light
const dark = EPUB_THEME_COLORS.dark

export const lightVariantText = createEpubTheme(
	light.foreground,
	light.background,
	light.mutedForeground,
	light.linkForeground,
)

export const darkVariantText = createEpubTheme(
	dark.foreground,
	dark.background,
	dark.mutedForeground,
	dark.linkForeground,
)

export const toFamilyName = (font: SupportedFont) => {
	switch (font) {
		case SupportedFont.Inter:
			return 'Inter'
		case SupportedFont.OpenDyslexic:
			return 'OpenDyslexic'
		case SupportedFont.AtkinsonHyperlegibleNext:
			return 'Atkinson Hyperlegible Next'
		case SupportedFont.Charis:
			return 'Charis'
		case SupportedFont.Literata:
			return 'Literata'
		case SupportedFont.Bitter:
			return 'Bitter'
		case SupportedFont.LibreBaskerville:
			return 'Libre Baskerville'
		case SupportedFont.Nunito:
			return 'Nunito'
		case SupportedFont.HinaMincho:
			return 'Hina Mincho'
		default:
			return font
	}
}
