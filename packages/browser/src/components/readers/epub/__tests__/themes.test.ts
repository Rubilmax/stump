import { DEFAULT_BOOK_PREFERENCES } from '@stump/client'
import { SupportedFont } from '@stump/graphql'

import { darkVariantText, lightVariantText } from '../themes'

it('uses readable EPUB defaults', () => {
	expect(lightVariantText).toEqual({
		'body, body *': {
			color: '#000000 !important',
			'background-color': 'transparent !important',
		},
		'html, body': { 'background-color': '#FFFFFF !important' },
		p: { 'font-size': 'unset' },
		'a, a *': {
			color: '#005A9C !important',
			'text-decoration': 'underline !important',
		},
		'blockquote, blockquote *': { color: '#4A4A4A !important' },
	})
	expect(darkVariantText).toEqual({
		'body, body *': {
			color: '#E8EDF4 !important',
			'background-color': 'transparent !important',
		},
		'html, body': { 'background-color': '#161719 !important' },
		p: { 'font-size': 'unset' },
		'a, a *': {
			color: '#4299E1 !important',
			'text-decoration': 'underline !important',
		},
		'blockquote, blockquote *': { color: '#A8ACB0 !important' },
	})
	expect(DEFAULT_BOOK_PREFERENCES).toMatchObject({
		fontFamily: SupportedFont.Literata,
		fontSize: 16,
		lineHeight: 1.5,
	})
})
