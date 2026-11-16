import { Nav } from "./components/Nav";
import { Hero } from "./components/Hero";
import { Features } from "./components/Features";
import { HowItWorks } from "./components/HowItWorks";
import { ProductDetail } from "./components/ProductDetail";
import { Cta } from "./components/Cta";
import { Footer } from "./components/Footer";

export function App() {
	return (
		<div className="min-h-screen overflow-x-clip">
			<a
				href="#top"
				className="sr-only focus:not-sr-only focus:fixed focus:left-4 focus:top-4 focus:z-[100] focus:border-2 focus:border-ink focus:bg-open focus:text-ink focus:px-4 focus:py-2"
			>
				Skip to content
			</a>

			<Nav />
			<main>
				<Hero />
				<Features />
				<HowItWorks />
				<ProductDetail />
				<Cta />
			</main>
			<Footer />
		</div>
	);
}
