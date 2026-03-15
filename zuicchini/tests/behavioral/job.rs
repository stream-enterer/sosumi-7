use zuicchini::scheduler::{EngineScheduler, Job, JobQueue, JobState};

#[test]
fn enqueue_transitions_to_waiting() {
    let mut sched = EngineScheduler::new();
    let mut queue = JobQueue::new();

    let job = Job::new(5.0, &mut sched);
    let sig = job.state_signal();
    let id = queue.enqueue(job, &mut sched);

    assert_eq!(queue.get(id).unwrap().state(), JobState::Waiting);
    assert!(!queue.is_empty());

    queue.abort_job(id, &mut sched);
    sched.remove_signal(sig);
}

#[test]
fn start_transitions_to_running() {
    let mut sched = EngineScheduler::new();
    let mut queue = JobQueue::new();

    let job = Job::new(1.0, &mut sched);
    let sig = job.state_signal();
    let id = queue.enqueue(job, &mut sched);
    queue.start_job(id, &mut sched);

    assert_eq!(queue.get(id).unwrap().state(), JobState::Running);
    assert!(!queue.is_empty());

    queue.succeed_job(id, &mut sched);
    sched.remove_signal(sig);
}

#[test]
fn succeed_transitions_to_success() {
    let mut sched = EngineScheduler::new();
    let mut queue = JobQueue::new();

    let job = Job::new(1.0, &mut sched);
    let sig = job.state_signal();
    let id = queue.enqueue(job, &mut sched);
    queue.start_job(id, &mut sched);
    queue.succeed_job(id, &mut sched);

    assert_eq!(queue.get(id).unwrap().state(), JobState::Success);
    assert!(queue.is_empty());

    sched.remove_signal(sig);
}

#[test]
fn fail_records_error_text() {
    let mut sched = EngineScheduler::new();
    let mut queue = JobQueue::new();

    let job = Job::new(1.0, &mut sched);
    let sig = job.state_signal();
    let id = queue.enqueue(job, &mut sched);
    queue.start_job(id, &mut sched);
    queue.fail_job(id, "out of memory".to_string(), &mut sched);

    let j = queue.get(id).unwrap();
    assert_eq!(j.state(), JobState::Error);
    assert_eq!(j.error_text(), "out of memory");
    assert!(queue.is_empty());

    sched.remove_signal(sig);
}

#[test]
fn abort_transitions_to_aborted() {
    let mut sched = EngineScheduler::new();
    let mut queue = JobQueue::new();

    let job = Job::new(1.0, &mut sched);
    let sig = job.state_signal();
    let id = queue.enqueue(job, &mut sched);
    queue.abort_job(id, &mut sched);

    assert_eq!(queue.get(id).unwrap().state(), JobState::Aborted);
    assert!(queue.is_empty());

    sched.remove_signal(sig);
}

#[test]
fn priority_ordering_highest_first() {
    let mut sched = EngineScheduler::new();
    let mut queue = JobQueue::new();

    let job_low = Job::new(1.0, &mut sched);
    let sig_low = job_low.state_signal();
    let id_low = queue.enqueue(job_low, &mut sched);

    let job_mid = Job::new(5.0, &mut sched);
    let sig_mid = job_mid.state_signal();
    let id_mid = queue.enqueue(job_mid, &mut sched);

    let job_high = Job::new(10.0, &mut sched);
    let sig_high = job_high.state_signal();
    let id_high = queue.enqueue(job_high, &mut sched);

    // start_next picks highest priority
    let started = queue.start_next(&mut sched);
    assert_eq!(started, Some(id_high));
    queue.succeed_job(id_high, &mut sched);

    // Next highest
    let started = queue.start_next(&mut sched);
    assert_eq!(started, Some(id_mid));
    queue.succeed_job(id_mid, &mut sched);

    // Lowest
    let started = queue.start_next(&mut sched);
    assert_eq!(started, Some(id_low));
    queue.succeed_job(id_low, &mut sched);

    assert!(queue.is_empty());

    sched.remove_signal(sig_low);
    sched.remove_signal(sig_mid);
    sched.remove_signal(sig_high);
}

#[test]
fn set_priority_reorders_waiting() {
    let mut sched = EngineScheduler::new();
    let mut queue = JobQueue::new();

    let job_a = Job::new(1.0, &mut sched);
    let sig_a = job_a.state_signal();
    let id_a = queue.enqueue(job_a, &mut sched);

    let job_b = Job::new(10.0, &mut sched);
    let sig_b = job_b.state_signal();
    let id_b = queue.enqueue(job_b, &mut sched);

    // Boost A above B
    queue.set_priority(id_a, 20.0);

    let started = queue.start_next(&mut sched);
    assert_eq!(started, Some(id_a));

    queue.abort_job(id_a, &mut sched);
    queue.abort_job(id_b, &mut sched);
    sched.remove_signal(sig_a);
    sched.remove_signal(sig_b);
}

#[test]
fn fail_all_running_and_waiting() {
    let mut sched = EngineScheduler::new();
    let mut queue = JobQueue::new();

    let job1 = Job::new(1.0, &mut sched);
    let sig1 = job1.state_signal();
    let id1 = queue.enqueue(job1, &mut sched);
    queue.start_job(id1, &mut sched);

    let job2 = Job::new(2.0, &mut sched);
    let sig2 = job2.state_signal();
    let id2 = queue.enqueue(job2, &mut sched);

    let job3 = Job::new(3.0, &mut sched);
    let sig3 = job3.state_signal();
    let id3 = queue.enqueue(job3, &mut sched);

    queue.fail_all("shutdown", &mut sched);

    assert_eq!(queue.get(id1).unwrap().state(), JobState::Error);
    assert_eq!(queue.get(id2).unwrap().state(), JobState::Error);
    assert_eq!(queue.get(id3).unwrap().state(), JobState::Error);
    assert_eq!(queue.get(id1).unwrap().error_text(), "shutdown");
    assert!(queue.is_empty());

    sched.remove_signal(sig1);
    sched.remove_signal(sig2);
    sched.remove_signal(sig3);
}

#[test]
fn waiting_and_running_job_lists() {
    let mut sched = EngineScheduler::new();
    let mut queue = JobQueue::new();

    let job1 = Job::new(1.0, &mut sched);
    let sig1 = job1.state_signal();
    let id1 = queue.enqueue(job1, &mut sched);

    let job2 = Job::new(2.0, &mut sched);
    let sig2 = job2.state_signal();
    let id2 = queue.enqueue(job2, &mut sched);

    // Both waiting
    let waiting = queue.waiting_jobs();
    assert_eq!(waiting.len(), 2);

    // Start one
    queue.start_job(id1, &mut sched);
    let running = queue.running_jobs();
    assert_eq!(running.len(), 1);
    assert_eq!(running[0], id1);

    let waiting = queue.waiting_jobs();
    assert_eq!(waiting.len(), 1);

    queue.abort_job(id1, &mut sched);
    queue.abort_job(id2, &mut sched);
    sched.remove_signal(sig1);
    sched.remove_signal(sig2);
}

#[test]
fn clear_aborts_all() {
    let mut sched = EngineScheduler::new();
    let mut queue = JobQueue::new();

    let job1 = Job::new(1.0, &mut sched);
    let sig1 = job1.state_signal();
    let id1 = queue.enqueue(job1, &mut sched);
    queue.start_job(id1, &mut sched);

    let job2 = Job::new(2.0, &mut sched);
    let sig2 = job2.state_signal();
    let id2 = queue.enqueue(job2, &mut sched);

    queue.clear(&mut sched);

    assert_eq!(queue.get(id1).unwrap().state(), JobState::Aborted);
    assert_eq!(queue.get(id2).unwrap().state(), JobState::Aborted);
    assert!(queue.is_empty());

    sched.remove_signal(sig1);
    sched.remove_signal(sig2);
}

#[test]
fn job_priority_and_signal_accessors() {
    let mut sched = EngineScheduler::new();
    let job = Job::new(7.5, &mut sched);
    assert_eq!(job.priority(), 7.5);
    assert_eq!(job.state(), JobState::NotEnqueued);
    assert_eq!(job.error_text(), "");

    let sig = job.state_signal();
    job.remove_signal(&mut sched);
    // Signal was removed (no panic)
    let _ = sig;
}

#[test]
fn start_next_returns_none_when_empty() {
    let mut sched = EngineScheduler::new();
    let mut queue = JobQueue::new();
    assert!(queue.start_next(&mut sched).is_none());
}

#[test]
fn first_waiting_and_running_none_when_empty() {
    let _sched = EngineScheduler::new();
    let mut queue = JobQueue::new();
    assert!(queue.first_waiting_job().is_none());
    assert!(queue.first_running_job().is_none());
}
