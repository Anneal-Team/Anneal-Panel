do $$
begin
    if to_regclass('apalis.jobs') is null then
        return;
    end if;

    if (
        select column_name
        from information_schema.columns
        where table_schema = 'apalis'
          and table_name = 'jobs'
        order by ordinal_position
        limit 1
    ) = 'job' then
        return;
    end if;

    drop trigger if exists notify_workers on apalis.jobs;
    drop function if exists apalis.notify_new_jobs();
    drop function if exists apalis.get_jobs(text, text, integer);
    drop table if exists apalis.jobs_legacy_layout_fix;

    alter table apalis.jobs rename to jobs_legacy_layout_fix;

    create table apalis.jobs (
        job bytea not null,
        id text not null,
        job_type text not null,
        status text not null default 'Pending',
        attempts integer not null default 0,
        max_attempts integer not null default 25,
        run_at timestamptz not null default now(),
        last_result jsonb null,
        lock_at timestamptz null,
        lock_by text null references apalis.workers(id),
        done_at timestamptz null,
        priority integer not null default 0,
        metadata jsonb null,
        primary key (id)
    );

    insert into apalis.jobs (
        job,
        id,
        job_type,
        status,
        attempts,
        max_attempts,
        run_at,
        last_result,
        lock_at,
        lock_by,
        done_at,
        priority,
        metadata
    )
    select
        job,
        id,
        job_type,
        status,
        attempts,
        max_attempts,
        run_at,
        last_result,
        lock_at,
        lock_by,
        done_at,
        priority,
        metadata
    from apalis.jobs_legacy_layout_fix;

    create index if not exists apalis_jobs_status_idx on apalis.jobs(status);
    create index if not exists apalis_jobs_lock_by_idx on apalis.jobs(lock_by);
    create index if not exists apalis_jobs_job_type_idx on apalis.jobs(job_type);

    drop table apalis.jobs_legacy_layout_fix;

    create or replace function apalis.get_jobs(
        worker_id text,
        v_job_type text,
        v_job_count integer default 5::integer
    ) returns setof apalis.jobs as $fn$
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
    $fn$ language plpgsql volatile;

    create function apalis.notify_new_jobs() returns trigger as $fn$
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
    $fn$ language plpgsql;

    create trigger notify_workers
    after insert on apalis.jobs
    for each row execute function apalis.notify_new_jobs();
end
$$;
