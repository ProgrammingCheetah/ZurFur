import { type FormEvent, useState } from "react";
import { startLogin, ApiError } from "../lib/api";

export default function LoginPage() {
  const [handle, setHandle] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleSubmit(e: FormEvent) {
    e.preventDefault();
    setError(null);

    const trimmed = handle.trim();
    if (!trimmed) {
      setError("Please enter your Bluesky handle.");
      return;
    }

    setLoading(true);
    try {
      const { redirect_url } = await startLogin(trimmed);
      window.location.href = redirect_url;
    } catch (err) {
      if (err instanceof ApiError) {
        if (err.status === 404) {
          setError("Could not find that handle or reach its server. Check the spelling and try again.");
        } else if (err.status === 502) {
          setError("Could not reach Bluesky servers. Please try again later.");
        } else {
          setError(err.message);
        }
      } else {
        setError("Something went wrong. Please try again.");
      }
      setLoading(false);
    }
  }

  return (
    <div className="login-container">
      <div className="login-card">
        <div className="login-header">
          <div className="logo-mark" aria-hidden="true">
            <span className="logo-letter">Z</span>
          </div>
          <h1>Zurfur</h1>
          <p className="tagline">Art commissions, powered by the open social web</p>
        </div>

        <form onSubmit={handleSubmit} className="login-form">
          <label htmlFor="handle-input" className="sr-only">
            Bluesky handle
          </label>
          <div className="input-group">
            <span className="input-prefix" aria-hidden="true">@</span>
            <input
              id="handle-input"
              type="text"
              value={handle}
              onChange={(e) => setHandle(e.target.value)}
              placeholder="yourname.bsky.social"
              autoComplete="username"
              autoFocus
              disabled={loading}
              aria-describedby={error ? "login-error" : undefined}
            />
          </div>

          {error && (
            <p id="login-error" className="error-message" role="alert">
              {error}
            </p>
          )}

          <button type="submit" className="login-button" disabled={loading}>
            {loading ? (
              <span className="spinner" aria-label="Signing in..." />
            ) : (
              <>
                Sign in with Bluesky
                <svg
                  className="bluesky-icon"
                  viewBox="0 0 568 501"
                  aria-hidden="true"
                  width="18"
                  height="18"
                >
                  <path
                    fill="currentColor"
                    d="M123.121 33.664C188.241 82.553 258.281 181.68 284 234.873c25.719-53.192 95.759-152.32 160.879-201.21C491.866-1.611 568-28.906 568 57.947c0 17.346-9.945 145.713-15.778 166.555-20.275 72.453-94.155 90.933-159.875 79.748C507.222 323.8 536.444 388.56 473.333 453.32c-119.86 122.992-172.272-30.859-185.702-70.281-2.462-7.227-3.614-10.608-3.631-7.733-.017-2.875-1.169.506-3.631 7.733-13.43 39.422-65.842 193.273-185.702 70.281-63.111-64.76-33.89-129.52 80.986-149.07-65.72 11.185-139.6-7.295-159.875-79.748C9.945 203.659 0 75.291 0 57.946 0-28.906 76.135-1.612 123.121 33.664Z"
                  />
                </svg>
              </>
            )}
          </button>
        </form>

        <p className="login-footer">
          Don't have a Bluesky account?{" "}
          <a href="https://bsky.app" target="_blank" rel="noopener noreferrer">
            Create one here
          </a>
        </p>
      </div>
    </div>
  );
}
