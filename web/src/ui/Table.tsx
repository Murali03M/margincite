import type { ReactNode } from "react";

export type Column<T> = {
	header: string;
	cell: (row: T) => ReactNode;
	/** Right-aligned, for numbers. */
	numeric?: boolean;
};

type TableProps<T> = {
	/** Read aloud for the table; not drawn. The screen's own heading is what people see. */
	caption: string;
	columns: Column<T>[];
	rows: T[];
	rowKey: (row: T) => string;
};

/**
 * A thin-bordered table. It scrolls sideways inside its own box, by keyboard too.
 */
export function Table<T>({ caption, columns, rows, rowKey }: TableProps<T>) {
	return (
		<div
			role="region"
			aria-label={caption}
			tabIndex={0}
			className="overflow-x-auto rounded-md border border-line bg-raised"
		>
			<table className="data-table">
				<caption className="sr-only">{caption}</caption>
				<thead>
					<tr>
						{columns.map((column) => (
							<th key={column.header} scope="col" className={"label " + (column.numeric ? "num" : "")}>
								{column.header}
							</th>
						))}
					</tr>
				</thead>
				<tbody>
					{rows.map((row) => (
						<tr key={rowKey(row)}>
							{columns.map((column) => (
								<td key={column.header} className={column.numeric ? "num" : undefined}>
									{column.cell(row)}
								</td>
							))}
						</tr>
					))}
				</tbody>
			</table>
		</div>
	);
}
