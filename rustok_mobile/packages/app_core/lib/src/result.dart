sealed class AppResult<T> {
  const AppResult();

  R when<R>({
    required R Function(T value) success,
    required R Function(AppError error) failure,
  }) {
    final self = this;
    if (self is AppSuccess<T>) {
      return success(self.value);
    }
    return failure((self as AppFailure<T>).error);
  }
}

final class AppSuccess<T> extends AppResult<T> {
  const AppSuccess(this.value);

  final T value;
}

final class AppFailure<T> extends AppResult<T> {
  const AppFailure(this.error);

  final AppError error;
}

class AppError {
  const AppError({required this.code, required this.message});

  final String code;
  final String message;
}
