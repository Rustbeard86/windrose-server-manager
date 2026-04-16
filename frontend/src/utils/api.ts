import type { ApiResponse } from '../types/api'

const API_BASE = ''

function getCookieValue(name: string): string | null {
  if (typeof document === 'undefined') return null
  const cookies = document.cookie ? document.cookie.split(';') : []
  for (const cookie of cookies) {
    const [rawKey, ...rest] = cookie.trim().split('=')
    if (rawKey === name) {
      return decodeURIComponent(rest.join('='))
    }
  }
  return null
}

function shouldAttachCsrf(method?: string): boolean {
  const m = (method ?? 'GET').toUpperCase()
  return !(m === 'GET' || m === 'HEAD' || m === 'OPTIONS')
}

export async function apiFetch<T>(path: string, options?: RequestInit): Promise<ApiResponse<T>> {
  const headers = new Headers(options?.headers)
  if (shouldAttachCsrf(options?.method)) {
    const csrf = getCookieValue('wsm_csrf')
    if (csrf) {
      headers.set('X-CSRF-Token', csrf)
    }
  }

  const response = await fetch(API_BASE + path, {
    ...options,
    credentials: 'include',
    headers,
  })
  if (!response.ok) {
    let apiMessage: string | null = null
    try {
      const payload = (await response.json()) as ApiResponse<unknown>
      apiMessage = payload?.message ?? null
    } catch {
      // ignore non-JSON error payloads
    }
    if (response.status === 401) {
      throw new Error(apiMessage ? `HTTP 401: ${apiMessage}` : 'HTTP 401: Unauthorized')
    }
    throw new Error(apiMessage ? `HTTP ${response.status}: ${apiMessage}` : `HTTP ${response.status}: ${response.statusText}`)
  }
  return response.json() as Promise<ApiResponse<T>>
}

export async function apiGet<T>(path: string): Promise<ApiResponse<T>> {
  return apiFetch<T>(path)
}

export async function apiPost<T>(path: string, body?: unknown): Promise<ApiResponse<T>> {
  return apiFetch<T>(path, {
    method: 'POST',
    headers: body !== undefined ? { 'Content-Type': 'application/json' } : undefined,
    body: body !== undefined ? JSON.stringify(body) : undefined,
  })
}

export async function apiPut<T>(path: string, body: unknown): Promise<ApiResponse<T>> {
  return apiFetch<T>(path, {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  })
}
