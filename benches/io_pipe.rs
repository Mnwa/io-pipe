use std::io::{IoSlice, Read, Write};

use criterion::{BatchSize, black_box, Criterion, criterion_group, criterion_main};

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("single writes", |b| {
        let (mut writer, reader) = io_pipe::pipe();

        b.iter(|| writer.write(black_box(&[0, 0, 0, 0])).unwrap());
        drop(reader)
    });
    c.bench_function("big batches write", |b| {
        let (mut writer, reader) = io_pipe::pipe();

        b.iter_batched(
            || vec![IoSlice::new(&[0, 0, 0, 0]); 100],
            |slice| writer.write_vectored(black_box(slice.as_slice())).unwrap(),
            BatchSize::LargeInput,
        );
        drop(reader)
    });
    c.bench_function("reads", |b| {
        let (mut writer, mut reader) = io_pipe::pipe();

        b.iter_batched(
            || {
                writer.write_all(&[0, 0, 0, 0]).unwrap();
                [0; 4]
            },
            |mut buf| assert_eq!(buf.len(), reader.read(black_box(&mut buf)).unwrap()),
            BatchSize::SmallInput,
        );
        drop(writer);

        assert_eq!(0, reader.read(&mut [0; 4]).unwrap())
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
