import { content } from "../content";
import { CostStamp, ServesTags } from "./Tags";

const features = content.features;
const cell = "block px-0 py-1 align-top md:table-cell md:px-4 md:py-6";

export function Features() {
	return (
		<section id="features" className="border-t border-ink bg-paper py-20 lg:py-28">
			<div className="wrap grid gap-x-14 gap-y-10 lg:grid-cols-12">
				<div className="lg:col-span-4">
					<div className="lg:sticky lg:top-24">
						<h2 className="heading">{features.heading}</h2>
						<p className="lead mt-5 text-ink-soft">{features.intro}</p>
					</div>
				</div>
				<div className="lg:col-span-8">
					<table className="w-full border-collapse text-left">
						<thead className="max-md:hidden">
							<tr className="border-b-2 border-ink">
								<th className="label px-4 pb-3 pl-0 text-ink-faint">{features.columns.what}</th>
								<th className="label px-4 pb-3 text-ink-faint">{features.columns.serves}</th>
								<th className="label px-4 pb-3 pr-0 text-ink-faint">{features.columns.needs}</th>
							</tr>
						</thead>
						<tbody>
							{features.items.map((item) => (
								<tr key={item.title} className="block border-b border-rule py-5 md:table-row md:py-0">
									<td className={cell + " md:pl-0"}>
										<h3 className="title">{item.title}</h3>
										<p className="mt-2 max-w-[34rem] text-ink-soft">{item.body}</p>
									</td>
									<td className={cell + " md:w-[8.5rem]"}>
										<ServesTags serves={item.serves} />
									</td>
									<td className={cell + " md:w-[13rem] md:pr-0"}>
										<span className="flex flex-wrap items-center gap-x-3 gap-y-2 md:flex-col md:items-start">
											<span className="text-[0.9375rem]">{item.needs}</span>
											<CostStamp cost={item.cost} />
										</span>
									</td>
								</tr>
							))}
						</tbody>
					</table>
				</div>
			</div>
		</section>
	);
}
