import { FormatStrip, Hero } from "@/components/Hero";
import { Nav } from "@/components/Nav";
import { ProductShot } from "@/components/ProductShot";
import { Faq } from "@/components/Faq";
import { Install } from "@/components/Install";
import { Compare, Footer, Speed, Tv } from "@/components/Sections";
import { Story } from "@/components/Story";

export default function Home() {
  return (
    <>
      <Nav />
      <main>
        <Hero />
        <ProductShot />
        <FormatStrip />
        <Story />
        <Speed />
        <Compare />
        <Tv />
        <Install />
        <Faq />
      </main>
      <Footer />
      <div className="grain" aria-hidden />
    </>
  );
}
