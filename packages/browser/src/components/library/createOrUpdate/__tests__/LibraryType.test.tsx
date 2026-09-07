import { LibraryType } from '@stump/graphql'
import { render, screen } from '@testing-library/react'
import { FormProvider, useForm } from 'react-hook-form'

import { CreateOrUpdateLibrarySchema } from '../schema'
import LibraryTypeSelect from '../sections/LibraryType'

vi.mock('@stump/i18n', () => ({
	useLocaleContext: () => ({ t: (key: string) => key }),
}))

function Subject() {
	const form = useForm<CreateOrUpdateLibrarySchema>({
		defaultValues: { libraryType: LibraryType.Mixed },
	})

	return (
		<FormProvider {...form}>
			<LibraryTypeSelect />
		</FormProvider>
	)
}

test('associates the label with the library type select', () => {
	render(<Subject />)

	expect(
		screen.getByRole('combobox', {
			name: 'createOrUpdateLibraryForm.fields.libraryType.label',
		}),
	).toHaveAttribute('id', 'library-type')
})
