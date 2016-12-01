$(() => {

    /**
     * Submit Verify a stamp form
     */
    $('#verify-stamp').on('submit', function(event) {
        var hash = $('#hash');

        // Remove all errors
        $('.has-error', $(event.target)).removeClass('has-error');

        if (!hash.val()) {
            hash.parents('.form-group').addClass('has-error');
            event.preventDefault();
        }
    });

});