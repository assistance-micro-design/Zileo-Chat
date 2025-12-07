/**
 * Async Handler Utilities
 *
 * Provides factory functions for creating standardized async operation handlers
 * with loading state management and error handling.
 *
 * @module utils/async
 *
 * @example
 * ```typescript
 * // Create a handler for saving data
 * const handleSave = createAsyncHandler(
 *   () => invoke('save_data', { data }),
 *   {
 *     onSuccess: (result) => {
 *       message = { type: 'success', text: 'Saved successfully' };
 *     },
 *     onError: (error) => {
 *       message = { type: 'error', text: `Failed to save: ${error}` };
 *     },
 *     setLoading: (loading) => {
 *       saving = loading;
 *     }
 *   }
 * );
 *
 * // Use in event handler
 * <button onclick={handleSave}>Save</button>
 * ```
 */

// Note: Error handling utilities are available from './error' if needed:
// import { getErrorMessage } from './error';

/**
 * Options for async handler behavior
 */
export interface AsyncHandlerOptions<T> {
	/**
	 * Called when the operation succeeds with the result.
	 * @param result - The result from the async operation
	 */
	onSuccess?: (result: T) => void;

	/**
	 * Called when the operation fails with the error.
	 * Uses getErrorMessage() to extract error message from any error type.
	 * @param error - The error that occurred
	 */
	onError?: (error: unknown) => void;

	/**
	 * Called to update loading state before/after operation.
	 * @param loading - Whether operation is in progress
	 */
	setLoading?: (loading: boolean) => void;

	/**
	 * Called regardless of success or failure (like finally block).
	 */
	onComplete?: () => void;
}

/**
 * Creates a wrapped async handler function that manages loading state and error handling.
 *
 * This factory eliminates the repetitive try/catch/finally pattern commonly used
 * in async event handlers. It automatically:
 * - Sets loading state before the operation
 * - Calls onSuccess with the result on success
 * - Calls onError with the error on failure
 * - Always resets loading state in finally
 *
 * @template T - The return type of the async operation
 * @param operation - The async operation to wrap
 * @param options - Callbacks for success, error, and loading state
 * @returns A function that can be used as an event handler
 *
 * @example
 * ```typescript
 * // Before: Verbose pattern repeated everywhere
 * async function handleDelete() {
 *   deleting = true;
 *   try {
 *     await invoke('delete_item', { id });
 *     message = { type: 'success', text: 'Deleted' };
 *   } catch (err) {
 *     message = { type: 'error', text: String(err) };
 *   } finally {
 *     deleting = false;
 *   }
 * }
 *
 * // After: Clean and consistent
 * const handleDelete = createAsyncHandler(
 *   () => invoke('delete_item', { id }),
 *   {
 *     setLoading: (l) => deleting = l,
 *     onSuccess: () => message = { type: 'success', text: 'Deleted' },
 *     onError: (e) => message = { type: 'error', text: getErrorMessage(e) }
 *   }
 * );
 * ```
 */
export function createAsyncHandler<T>(
	operation: () => Promise<T>,
	options: AsyncHandlerOptions<T>
): () => Promise<void> {
	return async () => {
		options.setLoading?.(true);
		try {
			const result = await operation();
			options.onSuccess?.(result);
		} catch (error) {
			options.onError?.(error);
		} finally {
			options.setLoading?.(false);
			options.onComplete?.();
		}
	};
}

/**
 * Creates an async handler that includes the original event parameter.
 *
 * Useful when you need access to the event object (e.g., for form data
 * or preventing default behavior).
 *
 * @template T - The return type of the async operation
 * @template E - The event type
 * @param operation - The async operation that receives the event
 * @param options - Callbacks for success, error, and loading state
 * @returns A function that can be used as an event handler
 *
 * @example
 * ```typescript
 * const handleSubmit = createAsyncHandlerWithEvent<void, SubmitEvent>(
 *   async (event) => {
 *     event.preventDefault();
 *     const formData = new FormData(event.currentTarget as HTMLFormElement);
 *     await invoke('submit_form', { data: Object.fromEntries(formData) });
 *   },
 *   {
 *     setLoading: (l) => submitting = l,
 *     onSuccess: () => closeModal()
 *   }
 * );
 * ```
 */
export function createAsyncHandlerWithEvent<T, E = Event>(
	operation: (event: E) => Promise<T>,
	options: AsyncHandlerOptions<T>
): (event: E) => Promise<void> {
	return async (event: E) => {
		options.setLoading?.(true);
		try {
			const result = await operation(event);
			options.onSuccess?.(result);
		} catch (error) {
			options.onError?.(error);
		} finally {
			options.setLoading?.(false);
			options.onComplete?.();
		}
	};
}

/**
 * Wraps an existing async function to add loading state management.
 *
 * Unlike createAsyncHandler, this preserves the original function signature
 * and return value. Useful for wrapping existing functions without changing
 * their API.
 *
 * @template T - The return type of the async function
 * @template Args - The argument types
 * @param fn - The async function to wrap
 * @param setLoading - Callback to update loading state
 * @returns The wrapped function with the same signature
 *
 * @example
 * ```typescript
 * const loadData = withLoadingState(
 *   async (id: string) => await invoke<Data>('get_data', { id }),
 *   (loading) => isLoading = loading
 * );
 *
 * // Usage - same signature as original
 * const data = await loadData('123');
 * ```
 */
export function withLoadingState<T, Args extends unknown[]>(
	fn: (...args: Args) => Promise<T>,
	setLoading: (loading: boolean) => void
): (...args: Args) => Promise<T> {
	return async (...args: Args): Promise<T> => {
		setLoading(true);
		try {
			return await fn(...args);
		} finally {
			setLoading(false);
		}
	};
}
