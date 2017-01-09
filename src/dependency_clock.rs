let dependent_clock = |sampled: Rc<RefCell<usize>>| {
    Box::new(Callback::new(Box::new(move |input, output| {
        let avail = input.len();
        output.write_from_ring(avail, input);
        *sampled.borrow_mut() += avail;
    })))
};

let dependency_clock = |sampled: Rc<RefCell<usize>>| {
    Box::new(Callback::new(Box::new(move |input, output| {
        let avail = {
            let mut sampled = sampled.borrow_mut();
            let avail = *sampled;
            *sampled = 0;
            avail
        };
        output.write_from_ring(avail, input);
    })))
};
