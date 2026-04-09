import { useEffect, useRef, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { exchangeCallback, ApiError } from "../lib/api";
import { storeSession, getAppUrl } from "../lib/auth";

export default function CallbackPage() {
  const [searchParams] = useSearchParams();
  const [error, setError] = useState<string | null>(null);
  const exchangedRef = useRef(false);

  useEffect(() => {
    if (exchangedRef.current) return;
    exchangedRef.current = true;

    const code = searchParams.get("code");
    const state = searchParams.get("state");
    const iss = searchParams.get("iss") ?? undefined;

    if (!code || !state) {
      setError("Missing authorization parameters. Please try logging in again.");
      return;
    }

    async function exchange() {
      try {
        const result = await exchangeCallback(code!, state!, iss);

        storeSession({
          accessToken: result.access_token,
          refreshToken: result.refresh_token,
          userId: result.user_id,
          did: result.did,
          handle: result.handle,
        });

        // TODO: redirect to main app once it exists
        // window.location.href = getAppUrl();
        setError(JSON.stringify(result, null, 2));
      } catch (err) {
        if (err instanceof ApiError) {
          if (err.status === 400) {
            setError("Login session expired. Please try again.");
          } else {
            setError(`Login failed: ${err.message}`);
          }
        } else {
          setError("Something went wrong during login. Please try again.");
        }
      }
    }

    exchange();
  }, [searchParams]);

  if (error) {
    return (
      <div className="login-container">
        <div className="login-card">
          <div className="callback-status">
            <div className="error-icon" aria-hidden="true">!</div>
            <h1>Login Failed</h1>
            <p className="error-message">{error}</p>
            <a href="/" className="login-button">Try Again</a>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="login-container">
      <div className="login-card">
        <div className="callback-status">
          <div className="spinner large" aria-label="Completing login..." />
          <p>Completing login...</p>
        </div>
      </div>
    </div>
  );
}
