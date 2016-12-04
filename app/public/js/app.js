$(function() {

    $('.navbar-nav a, .pseudo-link').bind('click', function(event) {
        var $link = $(this);
        var ahcnor = $link.attr('href').match(/#(.*)$/g);

        if (ahcnor === null) {
            return true;
        }

        var $target = $(ahcnor[0]);

        if ($target.length === 0) {
            return true;
        }

        event.preventDefault();

        // Scroll page to hash
        $('html, body').stop().animate({
            scrollTop: $target.offset().top
        }, 1000, 'easeInOutExpo');

        // Add hash to URL
        if (history.pushState) {
            history.pushState(null, null, ahcnor[0]);
        } else {
            location.hash = ahcnor[0];
        }
    });

    $('#verify').on('submit', function(event) {
        var hash = $('#hash').val();

        event.preventDefault();

        if (hash.length === 0) {
            return false;
        }

        window.location.replace('/f/' + hash);
    });

    $('#create').on('submit', function(event) {
        var content = $('#content');
        var description = $('#description');

        event.preventDefault();

        if (content.val().length === 0) {
            return false;
        }

        var data = new FormData();
        data.append('content', content[0].files[0]);
        data.append('description', description.val());

        $.ajax({
            type: 'POST',
            data: data,
            url: '/create',
            cache: false,
            contentType: false,
            processData: false,
            success: function(data) {
                window.location.replace(data.redirect);
            }
        });
    });

});
