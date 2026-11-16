import { content } from "../content";

const layers = content.layers;

export function Layers() {
	return (
		<section id="layers" className="py-20 lg:py-28">
			<div className="wrap">
				<h2 className="heading max-w-[48rem]">{layers.heading}</h2>
				<p className="lead measure mt-5 text-ink-soft">{layers.intro}</p>
				<table className="mt-12 w-full table-fixed border-collapse text-left">
					<thead className="max-md:hidden">
						<tr>
							<td className="w-[13rem]" />
							{layers.columns.map((column) => (
								<th key={column.name} scope="col" className="px-4 pb-5 align-bottom">
									<span className="inline-block rounded-[3px] bg-plate px-2.5 py-1 font-[family-name:var(--font-display)] text-[1.5rem] font-[850] leading-none text-marker [font-stretch:112%]">
										{column.name}
									</span>
									<span className="label mt-3 block text-ink-soft">{column.full}</span>
								</th>
							))}
						</tr>
					</thead>
					<tbody>
						{layers.rows.map((row) => (
							<tr key={row.label} className="block border-t-2 border-ink py-4 md:table-row md:py-0">
								<th scope="row" className="label block pb-2 align-top text-ink md:table-cell md:py-6 md:pr-4">
									{row.label}
								</th>
								{row.cells.map((text, i) => (
									<td key={layers.columns[i].name} className="block py-1.5 align-top md:table-cell md:px-4 md:py-6">
										<span className="mr-2 font-[family-name:var(--font-mono)] text-[0.8125rem] font-bold md:hidden">
											{layers.columns[i].name}
										</span>
										{text}
									</td>
								))}
							</tr>
						))}
					</tbody>
				</table>
			</div>
		</section>
	);
}
