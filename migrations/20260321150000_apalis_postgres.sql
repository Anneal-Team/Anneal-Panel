create schema if not exists apalis;

create table if not exists apalis.workers (
    id text primary key,
    worker_type text not null,
    storage_name text not null,
    layers text not null default '',
    last_seen timestamptz not null default now(),
    started_at timestamptz null
);

create index if not exists apalis_workers_worker_type_idx on apalis.workers(worker_type);
create index if not exists apalis_workers_last_seen_idx on apalis.workers(last_seen);

create table if not exists apalis.jobs (
    id text primary key,
    job_type text not null,
    job bytea not null,
    status text not null default 'Pending',
    attempts integer not null default 0,
    max_attempts integer not null default 25,
    run_at timestamptz not null default now(),
    lock_at timestamptz null,
    lock_by text null references apalis.workers(id),
    done_at timestamptz null,
    last_result jsonb null,
    priority integer not null default 0,
    metadata jsonb null
);

create index if not exists apalis_jobs_status_idx on apalis.jobs(status);
create index if not exists apalis_jobs_lock_by_idx on apalis.jobs(lock_by);
create index if not exists apalis_jobs_job_type_idx on apalis.jobs(job_type);

create or replace function apalis.get_jobs(
    worker_id text,
    v_job_type text,
    v_job_count integer default 5::integer
) returns setof apalis.jobs as $$
begin
    return query
    update apalis.jobs
    set status = 'Queued',
        lock_by = worker_id,
        lock_at = now()
    where id in (
        select id
        from apalis.jobs
        where (status = 'Pending' or (status = 'Failed' and attempts < max_attempts))
          and run_at < now()
          and job_type = v_job_type
        order by priority desc, run_at asc, id asc
        limit v_job_count
        for update skip locked
    )
    returning *;
end;
$$ language plpgsql volatile;

drop trigger if exists notify_workers on apalis.jobs;
drop function if exists apalis.notify_new_jobs;

create function apalis.notify_new_jobs() returns trigger as $$
begin
    if new.run_at <= now() then
        perform pg_notify(
            'apalis::job::insert',
            json_build_object(
                'job_type', new.job_type,
                'id', new.id,
                'run_at', new.run_at
            )::text
        );
    end if;
    return new;
end;
$$ language plpgsql;

create trigger notify_workers
after insert on apalis.jobs
for each row execute function apalis.notify_new_jobs();
