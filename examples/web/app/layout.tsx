export const metadata = {
  title: "Bullet SDK — Web Example",
  description: "WASM SDK running in Next.js with SSR + CSR via Turbopack",
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en">
      <body
        style={{
          fontFamily: "system-ui, sans-serif",
          padding: "2rem",
          maxWidth: "800px",
          margin: "0 auto",
        }}
      >
        <h1>Bullet SDK — Next.js Example</h1>
        <p style={{ color: "#666" }}>
          Demonstrates both server-side and client-side WASM usage
        </p>
        <hr />
        {children}
      </body>
    </html>
  );
}
