import type { ExtractStage } from './generated/ExtractStage';

const WEIGHTS = { templates: 4, locale: 4, schema: 2, images: 88, finish: 2 };
const ORDER = Object.keys(WEIGHTS) as ExtractStage[];

export interface RunProgress {
	stage: ExtractStage | null;
	done: number;
	total: number;
	fraction: number;
	percent: number;
	imagesSkipped: boolean;
}

export function initialProgress(): RunProgress {
	return { stage: null, done: 0, total: 0, fraction: 0, percent: 0, imagesSkipped: false };
}

/** Shared by channel events and status polling. Only a new run resets this state. */
export function advanceProgress(
	previous: RunProgress,
	stage: ExtractStage | null,
	done = 0,
	total = 0
): RunProgress {
	if (!stage) return previous;
	const index = ORDER.indexOf(stage);
	if (previous.stage && index < ORDER.indexOf(previous.stage)) return previous;
	const sameStage = stage === previous.stage;
	const imagesSkipped =
		previous.imagesSkipped || (stage === 'finish' && previous.stage === 'schema');
	const fraction = Math.max(
		sameStage ? previous.fraction : 0,
		total > 0 ? Math.max(0, Math.min(1, done / total)) : 0
	);
	// Keep the displayed pair together: a second series or out-of-order worker report
	// must not replace a larger count with a smaller one.
	const keepCount = sameStage && (done < previous.done || total < previous.total);
	const denominator = imagesSkipped ? 12 : 100;
	const finished = ORDER.slice(0, index).reduce(
		(sum, id) => sum + (imagesSkipped && id === 'images' ? 0 : WEIGHTS[id]),
		0
	);
	return {
		stage,
		done: keepCount ? previous.done : Math.max(0, done),
		total: keepCount ? previous.total : Math.max(0, total),
		fraction,
		percent: Math.min(
			100,
			Math.max(previous.percent, (100 * (finished + WEIGHTS[stage] * fraction)) / denominator)
		),
		imagesSkipped
	};
}
