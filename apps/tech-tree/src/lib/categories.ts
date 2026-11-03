// Display metadata for research categories (colors mirror `--category-*` in app.css).
import type { TechCategory } from '$lib/api';
import EnergyIcon from '$lib/assets/tech_energy_icon.png';
import InformationIcon from '$lib/assets/tech_info_icon.png';
import LifeIcon from '$lib/assets/tech_life_icon.png';
import MaterialIcon from '$lib/assets/tech_material_icon.png';
import MilitaryIcon from '$lib/assets/tech_military_icon.png';
import SocialIcon from '$lib/assets/tech_social_icon.png';
import SpaceIcon from '$lib/assets/tech_space_icon.png';
import XenologyIcon from '$lib/assets/tech_xeno_icon.png';

export interface CategoryInfo {
	label: string;
	icon: string;
	/** Hex color (used where CSS variables don't work, e.g. the MiniMap). */
	color: string;
	/** CSS variable reference for inline styles. */
	cssVar: string;
}

export const CATEGORIES: Record<TechCategory, CategoryInfo> = {
	Energy: {
		label: 'Energy',
		icon: EnergyIcon,
		color: '#FF6C00',
		cssVar: 'var(--category-energy)'
	},
	InformationScience: {
		label: 'Information Science',
		icon: InformationIcon,
		color: '#E75A73',
		cssVar: 'var(--category-information)'
	},
	LifeScience: {
		label: 'Life Science',
		icon: LifeIcon,
		color: '#3CC273',
		cssVar: 'var(--category-life)'
	},
	Materials: {
		label: 'Materials',
		icon: MaterialIcon,
		color: '#FDC549',
		cssVar: 'var(--category-materials)'
	},
	MilitaryScience: {
		label: 'Military Science',
		icon: MilitaryIcon,
		color: '#688C73',
		cssVar: 'var(--category-military)'
	},
	SocialScience: {
		label: 'Social Science',
		icon: SocialIcon,
		color: '#8CD8F4',
		cssVar: 'var(--category-social)'
	},
	SpaceScience: {
		label: 'Space Science',
		icon: SpaceIcon,
		color: '#0092DE',
		cssVar: 'var(--category-space)'
	},
	Xenology: {
		label: 'Xenology',
		icon: XenologyIcon,
		color: '#896BDE',
		cssVar: 'var(--category-xenology)'
	}
};

export const CATEGORY_ORDER = Object.keys(CATEGORIES) as TechCategory[];

export function categoryInfo(category: TechCategory | null | undefined): CategoryInfo | null {
	return category ? (CATEGORIES[category] ?? null) : null;
}
