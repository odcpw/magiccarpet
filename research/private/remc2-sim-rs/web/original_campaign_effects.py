"""One generation-checked native effect reader for browser presentation."""
import ctypes as c
from collections import deque

from original_gameplay_audio import event_head


class EventQuery(c.Structure):
    _fields_ = [('abi', c.c_uint32), ('size', c.c_uint32), ('generation', c.c_uint64),
                ('cursor', c.c_uint64), ('reserved', c.c_uint32 * 4)]


class EventBatch(c.Structure):
    _fields_ = [('abi', c.c_uint32), ('size', c.c_uint32), ('generation', c.c_uint64),
                ('next_cursor', c.c_uint64), ('head', c.c_uint64), ('written', c.c_uint32),
                ('has_more', c.c_uint8), ('reserved0', c.c_uint8 * 3), ('reserved', c.c_uint32 * 4)]


class CampaignEffect(c.Structure):
    _fields_ = [('abi', c.c_uint32), ('size', c.c_uint32), ('generation', c.c_uint64),
                ('sequence', c.c_uint64), ('state_hash', c.c_uint64), ('kind', c.c_uint32),
                ('code', c.c_uint32), ('entity', c.c_uint16), ('player', c.c_int16),
                ('value0', c.c_int32), ('value1', c.c_int32), ('asset', c.c_uint32),
                ('reserved', c.c_uint32 * 4)]


if (c.sizeof(EventQuery), c.sizeof(EventBatch), c.sizeof(CampaignEffect)) != (40, 56, 72):
    raise RuntimeError('unsupported native campaign effect ABI layout')


class CampaignEffects:
    def __init__(self, native):
        self.native = native
        self.generation = None
        self.cursor = 0
        self.notices = deque(maxlen=32)
        fn = native.lib.mc_native_campaign_events_copy_since
        fn.argtypes = [c.c_uint64, c.c_void_p, c.c_void_p, c.c_uint32, c.c_uint32, c.c_void_p, c.c_uint32]
        fn.restype = c.c_int32

    def reset(self):
        self.generation = None
        self.notices.clear()

    def poll(self, observation: bytes) -> list[CampaignEffect]:
        _, generation, _ = self.native.counters()
        head = event_head(observation)
        if self.generation != generation:
            self.generation, self.cursor = generation, head
            return []
        effects = []
        while self.cursor < head:
            query = EventQuery(2, c.sizeof(EventQuery), generation, self.cursor)
            rows, batch = (CampaignEffect * 256)(), EventBatch()
            self.native.check(self.native.lib.mc_native_campaign_events_copy_since(
                self.native.handle, c.byref(query), rows, len(rows), c.sizeof(CampaignEffect),
                c.byref(batch), c.sizeof(batch)), 'read campaign presentation effects')
            if (batch.abi, batch.size, batch.generation) != (2, 56, generation) or batch.written > len(rows) or \
                    batch.next_cursor != self.cursor + batch.written or batch.head != head or batch.has_more not in (0, 1) or \
                    (batch.written == 0 and self.cursor < head):
                raise RuntimeError('invalid native campaign effect page')
            for index in range(batch.written):
                row = rows[index]
                if (row.abi, row.size, row.generation, row.sequence) != (2, 72, generation, self.cursor + index + 1):
                    raise RuntimeError('invalid native campaign presentation effect')
                if row.kind == 0x101 and row.code == 5:
                    self.notices.append({'generation': str(generation), 'sequence': str(row.sequence),
                                         'kind': row.kind, 'code': row.code, 'value0': row.value0, 'value1': row.value1})
                if row.kind == 0x104:
                    effects.append(CampaignEffect.from_buffer_copy(row))
            self.cursor = batch.next_cursor
        return effects
