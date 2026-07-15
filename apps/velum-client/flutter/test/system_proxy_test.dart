import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:velum_client/system_proxy.dart';

void main() {
  test('enable snapshots before mutation and disable restores once', () async {
    final backend = _FakeBackend();
    final store = _MemoryStore();
    final proxy = SystemProxy(backend: backend, store: store);

    await proxy.enable(1080);
    expect(backend.events, ['capture', 'enable:1080']);
    expect(store.snapshot, isNotNull);

    await proxy.disable();
    expect(backend.events, ['capture', 'enable:1080', 'restore']);
    expect(store.snapshot, isNull);
  });

  test('failed enable restores the captured configuration', () async {
    final backend = _FakeBackend()..failEnable = true;
    final store = _MemoryStore();
    final proxy = SystemProxy(backend: backend, store: store);

    await expectLater(proxy.enable(1080), throwsStateError);
    expect(backend.events, ['capture', 'enable:1080', 'restore']);
    expect(store.snapshot, isNull);
  });

  test('failed recovery retains the backup for a later launch', () async {
    final backend = _FakeBackend()
      ..failEnable = true
      ..failRestore = true;
    final store = _MemoryStore();
    final proxy = SystemProxy(backend: backend, store: store);

    await expectLater(proxy.enable(1080), throwsStateError);
    expect(store.snapshot, isNotNull);
  });

  test('disable waits for an in-flight enable mutation', () async {
    final gate = Completer<void>();
    final backend = _FakeBackend()..enableGate = gate.future;
    final proxy = SystemProxy(backend: backend, store: _MemoryStore());

    final enabling = proxy.enable(1080);
    await Future<void>.delayed(Duration.zero);
    final disabling = proxy.disable();
    await Future<void>.delayed(Duration.zero);
    expect(backend.events, ['capture', 'enable:1080']);

    gate.complete();
    await Future.wait([enabling, disabling]);
    expect(backend.events, ['capture', 'enable:1080', 'restore']);
  });
}

final class _FakeBackend implements ProxyBackend {
  final events = <String>[];
  bool failEnable = false;
  bool failRestore = false;
  Future<void>? enableGate;

  @override
  String get id => 'fake';

  @override
  Future<ProxySnapshot> capture() async {
    events.add('capture');
    return const ProxySnapshot(backend: 'fake', values: {'mode': 'original'});
  }

  @override
  Future<void> enable(int port) async {
    events.add('enable:$port');
    await enableGate;
    if (failEnable) throw StateError('enable failed');
  }

  @override
  Future<void> restore(ProxySnapshot snapshot) async {
    events.add('restore');
    if (failRestore) throw StateError('restore failed');
  }
}

final class _MemoryStore implements ProxyBackupStore {
  ProxySnapshot? snapshot;

  @override
  Future<void> clear() async => snapshot = null;

  @override
  Future<ProxySnapshot?> read() async => snapshot;

  @override
  Future<void> write(ProxySnapshot value) async => snapshot = value;
}
